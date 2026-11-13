//! Abstract memory: allocations with byte images plus a shadow.
//!
//! Three spaces hold bytes: the *frame* space (locals and temporaries of
//! the running evaluation, a stack released by scope marks and reset after
//! every top-level evaluation), the *heap* space (transient allocations of
//! `new`/`delete`, spec/oven.md "Transient allocation": blocks reused
//! through size classes while the evaluation runs, reset with the frame
//! space) and the *static* space (thawed `constexpr` variables, string
//! literals and external objects; immutable, kept for the translation unit
//! and shared by later evaluations).
//!
//! Each space keeps, per 8-byte granule, a byte-granular initialization
//! mask (bit `i` = byte `i` of the granule holds a value) and a pointer
//! mark (nonzero: the granule holds a pointer, whose bytes are an encoded
//! [`Ptr`]; [`PTR_TAGGED`] and up: a `void *` whose designated type is
//! [`Memory::void_tag`] of the granule, spec/oven.md "Construction").
//! Allocations start on a granule boundary.
//!
//! A pointer is a `u64`: `offset (32) | past-end (1) | slot (22) | gen (9)`.
//! Null is 0 (frame slot 0 is never used). Slot numbers with bit 21 set
//! are static, those with bit 20 (and not 21) set are heap slots; the
//! all-ones slot is a function pointer whose offset is the `FuncId`. A
//! frame or heap slot's generation is bumped when the slot is reused, so a
//! dangling pointer never matches a later allocation in the same slot
//! (until then it designates the dead allocation, whose origin names it in
//! notes); a slot whose generation would wrap is retired. The last
//! generation is never reached: it marks a pointer that points at
//! nothing ([`NOWHERE`]).

use std::cell::Cell;
use std::collections::VecDeque;

use kiln_ast::{ConstValueId, DeclId, ExprId, QualType, StringLitId};
use kiln_source::fx::FxHashMap;

use crate::ty::{OTy, TyKind, Types};

mod snap;

pub(crate) const GRAN: u64 = 8;
const OFF_MASK: u64 = 0xFFFF_FFFF;
pub(crate) const PAST: u64 = 1 << 32;
const SLOT_SHIFT: u32 = 33;
const SLOT_MASK: u64 = (1 << 22) - 1;
const GEN_SHIFT: u32 = 55;
const GEN_MAX: u16 = (1 << 9) - 2;
/// The generation bits of a pointer that "points at nothing" (clang's
/// invalid designator): the result of a cast a constant expression may
/// not perform ([expr.const]/5.15), which a fold continues past
/// (`RunState::fold`, spec/oven.md "Pointers that point at nothing").
/// No allocation reaches this generation ([`GEN_MAX`]), so every access
/// through such a pointer fails the generation check, while null tests,
/// comparisons and arithmetic work on its slot and offset
/// ([`Ptr::nowhere_slots`]). Frame slot 0 with these bits is an integer
/// cast to a pointer, its offset the integer.
pub(crate) const NOWHERE: u64 = ((1 << 9) - 1) << GEN_SHIFT;
pub(crate) const STATIC_BIT: u32 = 1 << 21;
/// Marks a heap slot (frame slots are below it).
pub(crate) const HEAP_BIT: u32 = 1 << 20;
pub(crate) const FN_SLOT: u32 = (1 << 22) - 1;
/// The static slot of a pointer that points at nothing whose offset
/// indexes [`crate::Oven::synth`]: an address a C static initializer
/// computes that the encoding cannot hold (WP-1304).
pub(crate) const SYNTH_SLOT: u32 = FN_SLOT - 1;
/// The static slot of a pointer that points at nothing whose offset
/// indexes [`crate::Oven::label_diffs`]: the difference of two label
/// addresses a C static initializer computes (`&&a - &&b`, WP-1304).
pub(crate) const LABEL_DIFF_SLOT: u32 = FN_SLOT - 2;
/// [`Alloc::base`] of a static allocation that holds no bytes
/// ([`Memory::alloc_unbacked`]).
pub(crate) const UNBACKED: u32 = u32::MAX;
/// The first pointer mark of a tagged `void *` ([`Memory::set_void_tag`]).
pub(crate) const PTR_TAGGED: u8 = 2;
/// The pointer mark of a tagged `void *` whose type did not fit the
/// interned marks: its tag is in [`Memory::vspill`].
pub(crate) const PTR_SPILL: u8 = u8::MAX;

/// A decoded pointer.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct Ptr {
    pub slot: u32,
    pub generation: u16,
    pub past: bool,
    pub off: u32,
}

impl Ptr {
    pub fn decode(p: u64) -> Ptr {
        Ptr {
            slot: ((p >> SLOT_SHIFT) & SLOT_MASK) as u32,
            generation: (p >> GEN_SHIFT) as u16,
            past: p & PAST != 0,
            off: (p & OFF_MASK) as u32,
        }
    }

    pub fn encode(self) -> u64 {
        u64::from(self.off)
            | if self.past { PAST } else { 0 }
            | u64::from(self.slot) << SLOT_SHIFT
            | u64::from(self.generation) << GEN_SHIFT
    }

    pub fn is_fn(self) -> bool {
        self.slot == FN_SLOT
    }

    /// Static (or a function pointer, whose slot has every bit set).
    pub fn is_static(self) -> bool {
        self.slot & STATIC_BIT != 0
    }

    pub fn is_heap(self) -> bool {
        self.slot & (STATIC_BIT | HEAP_BIT) == HEAP_BIT
    }

    /// The space a (non-function) pointer's object lives in.
    pub fn space(self) -> Sp {
        if self.is_static() {
            Sp::Static
        } else if self.is_heap() {
            Sp::Heap
        } else {
            Sp::Frame
        }
    }

    /// The allocation identity (slot and generation), for side tables.
    pub fn alloc_key(p: u64) -> u64 {
        p >> SLOT_SHIFT
    }

    /// The key of the union subobject of type `ty` at byte `off` of
    /// `p`'s allocation in [`Memory::unions`].
    pub fn union_key(p: u64, off: u64, ty: OTy) -> UKey {
        UKey::new(Ptr::alloc_key(p), off, ty)
    }

    /// Whether raw pointer `p` points at nothing ([`NOWHERE`]).
    #[inline]
    pub fn is_nowhere(p: u64) -> bool {
        p & NOWHERE == NOWHERE
    }

    /// The slot of `p` (0 for null) if it or `q` points at nothing:
    /// such pointers compare by slot and offset (the generation of the
    /// one pointing at nothing is lost).
    pub fn nowhere_slots(p: u64, q: u64) -> Option<(u32, u32)> {
        if !Ptr::is_nowhere(p) && !Ptr::is_nowhere(q) {
            return None;
        }
        Some((Ptr::decode(p).slot, Ptr::decode(q).slot))
    }

    /// `p` with its offset replaced.
    pub fn with_off(p: u64, off: u64, past: bool) -> u64 {
        (p & !(OFF_MASK | PAST)) | (off & OFF_MASK) | if past { PAST } else { 0 }
    }
}

/// The key of a union subobject's entry in [`Memory::unions`]: its
/// allocation, its offset and its type. Nested unions can share an offset
/// (`union N { char c; union { int x; } q; }`: `N` and `q` both at 0), so
/// the offset alone does not name one (WP-224).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, kiln_snap::Snap)]
pub(crate) struct UKey {
    /// `alloc_key << 32 | offset`.
    pub at: u64,
    pub ty: OTy,
}

impl UKey {
    pub fn new(alloc_key: u64, off: u64, ty: OTy) -> UKey {
        UKey { at: alloc_key << 32 | off, ty }
    }

    /// The allocation identity ([`Ptr::alloc_key`]).
    pub fn alloc_key(self) -> u64 {
        self.at >> 32
    }
}

/// One of the three byte spaces.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, kiln_snap::Snap)]
pub(crate) enum Sp {
    Frame,
    Heap,
    Static,
}

/// The [`Memory::vtags`] key of granule `g` of space `sp`.
#[inline]
fn vkey(sp: Sp, g: u64) -> u64 {
    (sp as u64) << 62 | g
}

/// The pointer to [`crate::Oven::synth`] entry `idx` ([`SYNTH_SLOT`]).
pub(crate) fn synth_ptr(idx: u32) -> u64 {
    NOWHERE | Ptr { slot: SYNTH_SLOT, generation: 0, past: false, off: idx }.encode()
}

/// The [`crate::Oven::synth`] index `p` holds, if it is such a pointer.
pub(crate) fn synth_idx(p: u64) -> Option<u32> {
    let d = Ptr::decode(p);
    (Ptr::is_nowhere(p) && d.slot == SYNTH_SLOT).then_some(d.off)
}

/// The pointer to [`crate::Oven::label_diffs`] entry `idx`
/// ([`LABEL_DIFF_SLOT`]).
pub(crate) fn label_diff_ptr(idx: u32) -> u64 {
    NOWHERE | Ptr { slot: LABEL_DIFF_SLOT, generation: 0, past: false, off: idx }.encode()
}

/// The [`crate::Oven::label_diffs`] index `p` holds, if it is such a
/// pointer.
pub(crate) fn label_diff_idx(p: u64) -> Option<u32> {
    let d = Ptr::decode(p);
    (Ptr::is_nowhere(p) && d.slot == LABEL_DIFF_SLOT).then_some(d.off)
}

pub(crate) fn fn_ptr(func: u32) -> u64 {
    Ptr { slot: FN_SLOT, generation: 0, past: false, off: func }.encode()
}

/// What an allocation is, for diagnostics and freezing.
#[derive(Clone, Copy, PartialEq, Eq, Debug, kiln_snap::Snap)]
pub(crate) enum Origin {
    None,
    /// A local variable or parameter.
    Var(DeclId),
    /// A temporary materialized for an expression.
    Temp(ExprId),
    /// A namespace-scope or static variable whose value was thawed.
    Global(DeclId),
    /// A variable not usable in constant expressions (reads and writes
    /// are diagnosed; its address is a constant).
    External(DeclId),
    /// The object of unknown value a reference variable not usable in
    /// constant expressions binds to (P2280, C++23 [expr.const]/10): its
    /// address is usable, accesses are diagnosed (clang's
    /// constexpr-unknown lvalue).
    Unknown(DeclId),
    /// A string literal (and the expr that created it, if evaluated).
    Str(StringLitId, Option<ExprId>),
    /// An anonymous constant thawed from the store.
    Const(ConstValueId),
    /// A temporary with static storage duration, extended by a reference
    /// (its `LifetimeExtendedTemporary`): created by the extending
    /// variable's initializer, or thawed from `LetData.value` (external
    /// unless usable in constant expressions, [expr.const]/4).
    Let(DeclId),
    /// A transient allocation: the `new` expression (or the
    /// `std::allocator<T>::allocate` call) that made it, and its number in
    /// its evaluation context (clang's `{*new T#N}`; the leak check
    /// reports the lowest).
    Heap(ExprId, u32),
    /// The `std::type_info` object of a type (`typeid`, WP-077).
    TypeInfo(QualType),
    /// The object a GNU label address (`&&l`, its `AddrLabelExpr`)
    /// designates: an unbacked `void *`-typed object of unknown value
    /// (clang treats the expression as the base of an lvalue), one per
    /// expression (WP-444).
    Label(ExprId),
}

pub(crate) mod flags {
    pub const CONST: u8 = 1;
    pub const EXTERNAL: u8 = 2;
    pub const HAS_UNION: u8 = 4;
    /// A global whose initializer is being evaluated (a self-reference
    /// reads an object outside its lifetime).
    pub const RESOLVING: u8 = 8;
    /// A heap allocation made by an array form (`new[]`, `allocate`).
    pub const ARRAY_NEW: u8 = 16;
    /// A static object whose lifetime began outside the evaluation, with
    /// `mutable` subobjects: reads of them are diagnosed
    /// ([expr.const]/5.9, `Oven::mutable_path`).
    pub const MUTABLE_OUTSIDE: u8 = 32;
    /// A frame or heap object that is volatile, or holds a volatile
    /// member subobject: its accesses take `Oven::mutable_path`, which
    /// diagnoses one of a volatile (sub)object ([expr.const]/10.8,
    /// `Oven::volatile_path`, WP-1019). It shares its bit with
    /// [`MUTABLE_OUTSIDE`], which only static objects carry, so every
    /// fast path's existing bit test covers it.
    pub const VOLATILE: u8 = MUTABLE_OUTSIDE;
    /// A heap allocation made by `std::allocator<T>::allocate`: storage
    /// for `T[n]` whose elements are outside their lifetime until
    /// constructed (`Space::life`, spec/oven.md "Transient allocation").
    pub const STD_ALLOC: u8 = 64;
    /// A frame or `new` object one of whose subobjects' lifetime an
    /// explicit destruction ended: `Space::life` is kept for its bytes
    /// from then on (`Memory::set_lifetime`; spec/oven.md "Object
    /// lifetime outside `std::allocator` storage").
    pub const LIFE: u8 = 128;
    /// Storage whose bytes' lifetime [`Memory::check`] consults.
    pub const TRACKED: u8 = STD_ALLOC | LIFE;
}

/// How a heap object was allocated, and how one is deallocated (clang's
/// `DynAlloc::Kind`): the two must agree ([expr.delete]/2,
/// [allocator.members]/9).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum DynKind {
    /// `new T` / `delete p`.
    New,
    /// `new T[n]` / `delete[] p`.
    ArrayNew,
    /// `std::allocator<T>::allocate` / `deallocate`.
    Std,
}

impl DynKind {
    /// The kind of an allocation with flags `fl`.
    pub fn of(fl: u8) -> DynKind {
        if fl & flags::STD_ALLOC != 0 {
            DynKind::Std
        } else if fl & flags::ARRAY_NEW != 0 {
            DynKind::ArrayNew
        } else {
            DynKind::New
        }
    }
}

#[derive(Clone, Copy, Debug, kiln_snap::Snap)]
pub(crate) struct Alloc {
    pub base: u32,
    pub size: u32,
    pub generation: u16,
    pub live: bool,
    pub flags: u8,
    pub ty: OTy,
    pub origin: Origin,
    /// Frame objects: serial of the call frame that created it (0: none);
    /// tells a dead local of a live frame (scope ended) from one of a
    /// returned call. Heap objects: the evaluation context
    /// ([`Memory::ctx`]) that allocated it. Static objects: [`LAX_READ`]
    /// for the value of a variable only a lax fold may read, else 0.
    pub frame: u32,
}

/// [`Alloc::frame`] of a static [`flags::EXTERNAL`] object that holds the
/// value of a variable not usable in constant expressions but readable by
/// a lax fold ([`Memory::lax`]; spec/oven.md "Lax folds").
pub(crate) const LAX_READ: u32 = 1;

#[derive(Debug, Default, kiln_snap::Snap)]
pub(crate) struct Space {
    pub bytes: Vec<u8>,
    /// Per granule: bit i = byte i is initialized.
    pub init: Vec<u8>,
    /// Per granule: holds a pointer (nonzero; [`PTR_TAGGED`] and up: a
    /// tagged `void *`, [`Memory::void_tag`]).
    pub ptr: Vec<u8>,
    /// Per granule: bit i = byte i is within the lifetime of an object.
    /// Meaningful for [`flags::TRACKED`] allocations only, and empty
    /// until the first one (spec/oven.md "Transient allocation").
    pub life: Vec<u8>,
    pub top: u64,
}

// ---- per-granule bitmasks ----------------------------------------------------
//
// `Space::init` and `Space::life` keep one bit per byte: bit `i` of map
// byte `g` is byte `g * GRAN + i`. Read little-endian, the eight map bytes
// from map byte `g` are one `u64` whose bit `k` is byte `g * GRAN + k`, so
// the bits of a run of up to [`WBITS`] bytes are one shift and mask of one
// word, whatever its alignment.

/// The most bits [`get_bits`] and [`put_bits`] move at once: a word of
/// eight map bytes less the at most 7-bit offset into its first byte.
const WBITS: u64 = 56;

/// Ones in the low `n` bits (`n <= WBITS`).
#[inline]
fn ones(n: u64) -> u64 {
    (1u64 << n) - 1
}

/// The eight map bytes from byte `b` of `v` as a little-endian word (zero
/// past the end of `v`).
#[inline]
fn word(v: &[u8], b: usize) -> u64 {
    if let Some(w) = v.get(b..b + 8) {
        return u64::from_le_bytes(w.try_into().unwrap_or_default());
    }
    let mut w = [0u8; 8];
    let tail = v.get(b..).unwrap_or_default();
    w[..tail.len()].copy_from_slice(tail);
    u64::from_le_bytes(w)
}

/// The bits of bytes `[pos, pos + n)` in `v`, in the low `n` bits
/// (`n <= WBITS`).
#[inline]
fn get_bits(v: &[u8], pos: u64, n: u64) -> u64 {
    word(v, (pos / GRAN) as usize) >> (pos % GRAN) & ones(n)
}

/// Sets the bits of bytes `[pos, pos + n)` in `v` to the low `n` bits of
/// `x` (`n <= WBITS`), leaving every other bit as it is.
#[inline]
fn put_bits(v: &mut [u8], pos: u64, n: u64, x: u64) {
    if n == 0 {
        return;
    }
    let (b, sh) = ((pos / GRAN) as usize, pos % GRAN);
    let m = ones(n) << sh;
    let w = (word(v, b) & !m | x << sh & m).to_le_bytes();
    // The whole word when it fits (its other bits are written back
    // unchanged), else only the map bytes the range touches.
    match v.get_mut(b..b + 8) {
        Some(d) => d.copy_from_slice(&w),
        None => {
            let nb = (sh + n).div_ceil(GRAN) as usize;
            v[b..b + nb].copy_from_slice(&w[..nb]);
        }
    }
}

/// Splits `[pos, pos + len)` (reaching past the granule of `pos`) at
/// map-byte boundaries: the head's bit count, the first whole map byte,
/// the number of whole map bytes, and the tail's bit count.
#[inline]
fn split(pos: u64, len: u64) -> (u64, usize, usize, u64) {
    let h = pos.next_multiple_of(GRAN) - pos;
    let full = (len - h) / GRAN;
    (h, ((pos + h) / GRAN) as usize, full as usize, len - h - full * GRAN)
}

/// The mask of bits `[lo, hi)` of a map byte (`lo <= hi <= GRAN`).
#[inline]
fn byte_mask(lo: u64, hi: u64) -> u8 {
    (((1u32 << hi) - 1) & !((1u32 << lo) - 1)) as u8
}

/// Whether every byte of `[abs, abs + len)` has its bit set in the
/// per-granule bitmask `v`.
#[inline]
fn all_bits(v: &[u8], abs: u64, len: u64) -> bool {
    // Almost every access lies within one granule.
    let lo = abs % GRAN;
    if lo + len <= GRAN {
        let m = byte_mask(lo, lo + len);
        return m == 0 || v[(abs / GRAN) as usize] & m == m;
    }
    let (h, g, full, t) = split(abs, len);
    let part = |pos: u64, n: u64| {
        let m = byte_mask(pos % GRAN, pos % GRAN + n);
        m == 0 || v[(pos / GRAN) as usize] & m == m
    };
    part(abs, h) && v[g..g + full].iter().all(|&b| b == 0xFF) && part(abs + len - t, t)
}

/// Whether some byte of `[abs, abs + len)` has its bit set in the
/// per-granule bitmask `v`.
#[inline]
fn any_bits(v: &[u8], abs: u64, len: u64) -> bool {
    let lo = abs % GRAN;
    if lo + len <= GRAN {
        let m = byte_mask(lo, lo + len);
        return m != 0 && v[(abs / GRAN) as usize] & m != 0;
    }
    let (h, g, full, t) = split(abs, len);
    let part = |pos: u64, n: u64| {
        let m = byte_mask(pos % GRAN, pos % GRAN + n);
        m != 0 && v[(pos / GRAN) as usize] & m != 0
    };
    part(abs, h) || v[g..g + full].iter().any(|&b| b != 0) || part(abs + len - t, t)
}

/// Sets (`on`) or clears the bits of `[abs, abs + len)` in the
/// per-granule bitmask `v`.
#[inline]
fn set_bits(v: &mut [u8], abs: u64, len: u64, on: bool) {
    let lo = abs % GRAN;
    if lo + len <= GRAN {
        if len != 0 {
            let (b, m) = (&mut v[(abs / GRAN) as usize], byte_mask(lo, lo + len));
            if on { *b |= m } else { *b &= !m }
        }
        return;
    }
    set_bits_slow(v, abs, len, on);
}

/// [`set_bits`] of a range over more than one granule.
fn set_bits_slow(v: &mut [u8], abs: u64, len: u64, on: bool) {
    let (h, g, full, t) = split(abs, len);
    let mut put = |pos: u64, n: u64| {
        if n != 0 {
            let (b, m) = (&mut v[(pos / GRAN) as usize], byte_mask(pos % GRAN, pos % GRAN + n));
            if on { *b |= m } else { *b &= !m }
        }
    };
    put(abs, h);
    put(abs + len - t, t);
    v[g..g + full].fill(if on { 0xFF } else { 0 });
}

/// Copies the bits of bytes `[src, src + len)` of `s` to bytes
/// `[dst, dst + len)` of `d` (another map).
fn copy_bits(d: &mut [u8], dst: u64, s: &[u8], src: u64, len: u64) {
    if len > WBITS && src % GRAN == dst % GRAN {
        // The same offset into a map byte: whole map bytes move as bytes.
        let (h, sg, full, t) = split(src, len);
        let dg = ((dst + h) / GRAN) as usize;
        d[dg..dg + full].copy_from_slice(&s[sg..sg + full]);
        put_bits(d, dst, h, get_bits(s, src, h));
        put_bits(d, dst + len - t, t, get_bits(s, src + len - t, t));
        return;
    }
    let mut i = 0;
    while i < len {
        let n = (len - i).min(WBITS);
        put_bits(d, dst + i, n, get_bits(s, src + i, n));
        i += n;
    }
}

/// [`copy_bits`] within one map, as if every source bit were read before
/// any destination bit is written (the ranges may overlap).
fn copy_bits_within(v: &mut [u8], dst: u64, src: u64, len: u64) {
    if len > WBITS && src % GRAN == dst % GRAN {
        let (h, sg, full, t) = split(src, len);
        let (hb, tb) = (get_bits(v, src, h), get_bits(v, src + len - t, t));
        v.copy_within(sg..sg + full, ((dst + h) / GRAN) as usize);
        put_bits(v, dst, h, hb);
        put_bits(v, dst + len - t, t, tb);
        return;
    }
    // Chunk by chunk, away from the overlap: a chunk's write reaches no
    // source bit a later chunk reads.
    if dst <= src {
        let mut i = 0;
        while i < len {
            let n = (len - i).min(WBITS);
            put_bits(v, dst + i, n, get_bits(v, src + i, n));
            i += n;
        }
    } else {
        let mut i = len;
        while i > 0 {
            let n = i.min(WBITS);
            i -= n;
            put_bits(v, dst + i, n, get_bits(v, src + i, n));
        }
    }
}

impl Space {
    fn grow(&mut self, end: u64) {
        let need = end as usize;
        if self.bytes.len() < need {
            let n = need.max(self.bytes.len() * 2).next_multiple_of(GRAN as usize);
            self.bytes.resize(n, 0);
            self.init.resize(n / GRAN as usize, 0);
            self.ptr.resize(n / GRAN as usize, 0);
            if !self.life.is_empty() {
                self.life.resize(n / GRAN as usize, 0);
            }
        }
    }

    /// Starts keeping [`Space::life`].
    fn ensure_life(&mut self) {
        if self.life.is_empty() {
            self.life.resize(self.init.len().max(1), 0);
        }
    }

    /// Whether every byte of the range is within an object's lifetime.
    pub fn is_live(&self, abs: u64, len: u64) -> bool {
        all_bits(&self.life, abs, len)
    }

    /// Marks the range within (`v`) or outside an object's lifetime.
    pub fn set_life(&mut self, abs: u64, len: u64, v: bool) {
        set_bits(&mut self.life, abs, len, v);
    }

    pub fn is_init(&self, abs: u64, len: u64) -> bool {
        all_bits(&self.init, abs, len)
    }

    /// Whether some byte of the range is initialized.
    pub fn any_init(&self, abs: u64, len: u64) -> bool {
        any_bits(&self.init, abs, len)
    }

    pub fn set_init(&mut self, abs: u64, len: u64, v: bool) {
        set_bits(&mut self.init, abs, len, v);
    }

    /// Whether any granule overlapping the range holds a pointer.
    pub fn any_ptr(&self, abs: u64, len: u64) -> bool {
        let g0 = abs / GRAN;
        let g1 = (abs + len.max(1) - 1) / GRAN;
        (g0..=g1).any(|g| self.ptr[g as usize] != 0)
    }

    pub fn set_ptr(&mut self, abs: u64, len: u64, v: bool) {
        let g0 = abs / GRAN;
        let g1 = (abs + len.max(1) - 1) / GRAN;
        self.ptr[g0 as usize..=g1 as usize].fill(u8::from(v));
    }

    /// The pointer mark of the granule holding `abs`.
    pub fn ptr_mark(&self, abs: u64) -> u8 {
        self.ptr[(abs / GRAN) as usize]
    }

    /// Sets the pointer mark of the granules overlapping the range.
    pub fn set_ptr_mark(&mut self, abs: u64, len: u64, m: u8) {
        let g0 = abs / GRAN;
        let g1 = (abs + len.max(1) - 1) / GRAN;
        self.ptr[g0 as usize..=g1 as usize].fill(m);
    }

    /// The `len` bytes at `abs` as a little-endian value (the first 16
    /// when `len` is more).
    #[inline]
    pub fn read(&self, abs: u64, len: u64) -> u128 {
        // Up to 8 bytes: one 8-byte load, masked to `len`, unless it would
        // run past the end of the space.
        let a = abs as usize;
        if len <= 8
            && let Some(w) = self.bytes.get(a..a + 8)
        {
            let x = u64::from_le_bytes(w.try_into().unwrap_or_default());
            return u128::from(if len == 8 { x } else { x & ((1u64 << (8 * len)) - 1) });
        }
        let mut v = 0u128;
        for i in (0..len).rev() {
            v = v << 8 | u128::from(self.bytes[(abs + i) as usize]);
        }
        v
    }

    /// Writes the low `len` bytes of `v` at `abs`, little-endian (zeros
    /// past the 16th).
    #[inline]
    pub fn write(&mut self, abs: u64, len: u64, mut v: u128) {
        let a = abs as usize;
        if len <= 8
            && let Some(d) = self.bytes.get_mut(a..a + 8)
        {
            let m = if len == 8 { u64::MAX } else { (1u64 << (8 * len)) - 1 };
            let old = u64::from_le_bytes((&*d).try_into().unwrap_or_default());
            d.copy_from_slice(&(old & !m | v as u64 & m).to_le_bytes());
            return;
        }
        for i in 0..len {
            self.bytes[(abs + i) as usize] = v as u8;
            v >>= 8;
        }
    }
}

/// Why a memory operation failed (rendered into a note by the VM).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum MemFault {
    Null,
    FnPtr,
    /// Dead allocation (slot not reused yet: origin known).
    Dead(Origin),
    /// Stale generation (slot reused).
    Stale,
    PastEnd,
    OutOfBounds,
    Uninit,
    Const(OTy),
    /// Modification of a string literal object (its creating expr).
    Literal(Option<ExprId>),
    /// Modification of a namespace-scope or static variable.
    ModifyGlobal,
    /// Read (`false`) or write (`true`) of an external variable.
    External(DeclId, bool),
    /// An access of the object of unknown value of a reference variable
    /// ([`Origin::Unknown`]).
    Unknown(DeclId),
    /// Access of a static temporary not usable in constant expressions
    /// outside the evaluation that created it (its
    /// `LifetimeExtendedTemporary`).
    Temporary(DeclId),
    /// A pointer read from non-pointer bytes or vice versa.
    Incompatible,
    Memory(u64),
    /// `delete` of a pointer that is not to a heap allocation.
    NotHeap,
    /// `delete` of an allocation already deleted.
    DoubleDelete,
    /// `delete` of a pointer to a subobject of a heap allocation (or one
    /// past its end).
    Subobject,
    /// The wrong deallocation form for how the object was allocated.
    DeleteMismatch {
        alloc: DynKind,
        dealloc: DynKind,
    },
    /// `std::allocator<T>::deallocate` of a null pointer.
    DeallocNull,
    /// An access of `std::allocator` storage not within an object's
    /// lifetime (before its construction or after its destruction).
    Lifetime,
    /// An effect a speculative trial ([`TrialMode::Speculative`]) does
    /// not allow: modifying a frame or static object that existed at its
    /// checkpoint, or a heap (de)allocation in its evaluation context.
    Speculative,
}

/// Dead frame slots kept from reuse: a pointer to a just-ended object
/// (a callee's local after a return or an unwinding, a block's local
/// after its scope, whose slot the next frame object would take) is then
/// still "a variable whose lifetime has ended" with its declaration, not
/// a stale slot (clang's notes; spec/oven.md "Exceptions").
const FRAME_QUARANTINE: usize = 16;

#[derive(Debug)]
pub(crate) struct Memory {
    pub frame: Space,
    pub heap: Space,
    pub stat: Space,
    pub slots: Vec<Alloc>,
    /// Dead slots, reused first-in first-out, and only once more than
    /// [`FRAME_QUARANTINE`] wait, so a dangling pointer stays diagnosable
    /// (its slot keeps its origin) for as long as possible.
    free: VecDeque<u32>,
    pub live: Vec<u32>,
    pub statics: Vec<Alloc>,
    /// Dead static slots ([`Memory::free_static`]), reused (with the next
    /// generation) by [`Memory::alloc_static`] outside a trial.
    sfree: Vec<u32>,
    /// Free ranges of the static space below `stat.top` (base, length),
    /// sorted by base and coalesced: what dead static objects held.
    sholes: Vec<(u64, u64)>,
    /// The most static space ever held (`Memory::static_peak`).
    stat_peak: u64,
    /// Heap allocations, by slot minus [`HEAP_BIT`].
    pub hslots: Vec<Alloc>,
    /// Dead heap slots (first-in first-out, as `free`).
    hfree: VecDeque<u32>,
    /// Heap blocks free for reuse.
    bins: Bins,
    /// Granule-rounded bytes of the live heap allocations.
    pub heap_live: u64,
    /// Number of live heap allocations.
    pub heap_count: u32,
    /// Heap allocations made (cumulative; the `allocs` counter).
    pub heap_allocs: u64,
    /// Heap allocations and deallocations made (a call that made one is not
    /// memoized).
    pub heap_ops: u64,
    /// The running evaluation context (0: the outermost; stamped on heap
    /// allocations in `Alloc::frame`).
    pub ctx: u32,
    /// Active member of each union subobject with one: key the
    /// subobject ([`UKey`]), value the field index. Every change to it or
    /// to [`Memory::union_alts`] bumps [`Memory::union_epoch`].
    pub unions: FxHashMap<UKey, u32>,
    /// The members that may equally be the active one of a union whose
    /// member a placement new constructed through a pointer that names
    /// several of them (members of one type at one offset: a pointer is an
    /// allocation and an offset, without clang's designator), bit `i` for
    /// field `i` (< 64). [`Memory::unions`] holds one of them; an access
    /// or assignment naming another is accepted (WP-233). Cleared with the
    /// entry by [`Memory::union_put`].
    pub union_alts: FxHashMap<UKey, u64>,
    /// Bumped by every change to [`Memory::unions`] or
    /// [`Memory::union_alts`]: the union-path memo (`vm::umemo`, WP-760)
    /// trusts an answer only while the epoch it was computed at is
    /// current.
    pub union_epoch: u64,
    /// The last `true` answer of [`Memory::may_be_active`], with the
    /// [`Memory::union_epoch`] it holds at (`u64::MAX`: none): a member
    /// access checks the same union's member again and again (WP-760).
    active_memo: Cell<(UKey, u32, u64)>,
    /// The designated types of the `void *` values stored to memory
    /// (`kbc::VOID_TAG`), interned: pointer mark `PTR_TAGGED + i` is
    /// `vtags[i]`. Append-only for the translation unit (a rolled-back or
    /// copied mark stays valid) and at most `PTR_SPILL - PTR_TAGGED`
    /// long.
    vtags: Vec<OTy>,
    vtag_ix: FxHashMap<OTy, u8>,
    /// The designated types of the `void *` values stored once
    /// [`Memory::vtags`] is full (mark [`PTR_SPILL`]), keyed by granule
    /// (`sp << 62 | granule`, as [`Trial`]'s `seen`): no cap on distinct
    /// types (spec/oven.md "Construction"). An entry is meaningful while
    /// its granule's mark is [`PTR_SPILL`]; one whose granule was since
    /// overwritten is stale and harmless (the next spilled store there
    /// replaces it), so writes never clear it.
    vspill: FxHashMap<u64, OTy>,
    /// Bytes allocated (cumulative, granule-rounded; frame and heap).
    pub allocated: u64,
    /// Peak live bytes: `frame.top + heap_live`.
    pub peak: u64,
    /// Peak address space held: `frame.top + heap.top`.
    pub arena_peak: u64,
    /// The largest [`Memory::arena_peak`] of the translation unit so far
    /// ([`Memory::heap_peak`]): folded in when an evaluation ends, never
    /// on the VM's paths, and never lowered (WP-1315).
    tu_arena_peak: u64,
    /// Bytes the running evaluation added to the translation unit's
    /// reflection table and interned constant store (`refl_bytes`;
    /// spec/oven.md "Reflection bytes"): live for the rest of the
    /// translation unit, so part of the live bytes and the address space
    /// the memory limit bounds.
    pub refl: u64,
    pub limit: u64,
    /// Serial of the running call frame, and the last serial handed out.
    pub cur_frame: u32,
    pub serial: u32,
    /// A frame object has been [`flags::LIFE`]-tracked in this evaluation:
    /// leaving a scope checks the objects it releases (`Oven::scope_life`).
    pub life_frame: bool,
    /// One past the greatest index into [`Memory::live`] of a
    /// [`flags::LIFE`]-tracked frame object, or more: leaving a scope whose
    /// mark is at or above it releases no tracked object, and
    /// `Oven::scope_life` returns at once (WP-760).
    pub life_hi: usize,
    /// A lax fold is running (`Oven::fold_rvalue`): a [`LAX_READ`] object
    /// may be read.
    pub lax: bool,
    /// The base subobjects a constructor did not construct
    /// ([`Memory::set_unconstructed`]) in this evaluation, by pointer:
    /// their bytes are outside their lifetime while their derived object
    /// is not ([`Memory::holds_unconstructed`]); an empty base (`true`)
    /// has no bytes and is found by identity
    /// ([`Memory::empty_unconstructed`]). Empty unless Sema diagnosed a
    /// constructor (WP-390, WP-401).
    pub unconstructed: Vec<(u64, OTy, bool)>,
    /// Open checkpoints and their undo log (spec/oven.md "Trial
    /// evaluation").
    trial: Trial,
}

const DEAD0: Alloc = Alloc {
    base: 0,
    size: 0,
    generation: 0,
    live: false,
    flags: 0,
    ty: OTy(0),
    origin: Origin::None,
    frame: 0,
};

impl Memory {
    pub fn new(limit: u64) -> Self {
        Memory {
            frame: Space::default(),
            heap: Space::default(),
            stat: Space::default(),
            slots: vec![DEAD0],
            free: VecDeque::new(),
            live: Vec::new(),
            statics: Vec::new(),
            sfree: Vec::new(),
            sholes: Vec::new(),
            stat_peak: 0,
            hslots: Vec::new(),
            hfree: VecDeque::new(),
            bins: Bins::default(),
            heap_live: 0,
            heap_count: 0,
            heap_allocs: 0,
            heap_ops: 0,
            ctx: 0,
            unions: FxHashMap::default(),
            union_alts: FxHashMap::default(),
            union_epoch: 0,
            active_memo: Cell::new((UKey { at: 0, ty: OTy(0) }, 0, u64::MAX)),
            vtags: Vec::new(),
            vtag_ix: FxHashMap::default(),
            vspill: FxHashMap::default(),
            allocated: 0,
            peak: 0,
            arena_peak: 0,
            tu_arena_peak: 0,
            refl: 0,
            limit,
            cur_frame: 0,
            serial: 0,
            life_frame: false,
            life_hi: 0,
            lax: false,
            unconstructed: Vec::new(),
            trial: Trial::default(),
        }
    }

    /// The pointer mark of a pointer about to be stored at `abs` of space
    /// `sp` with register tag bits `tag` (`v >> 64`, `kbc::VOID_TAG`: a
    /// type + 1, or 0): 1 when untagged, else the tag's interned mark, or
    /// [`PTR_SPILL`] once every mark is taken, the tag then going to
    /// [`Memory::vspill`]. The caller logged the granule under the open
    /// checkpoints (its old spilled tag with it), so this is not logged
    /// again.
    #[inline]
    pub fn set_void_tag(&mut self, sp: Sp, abs: u64, tag: u32) -> u8 {
        let Some(t) = tag.checked_sub(1).map(OTy) else { return 1 };
        if let Some(&m) = self.vtag_ix.get(&t) {
            return m;
        }
        let m = self.vtags.len() + usize::from(PTR_TAGGED);
        if m < usize::from(PTR_SPILL) {
            self.vtags.push(t);
            self.vtag_ix.insert(t, m as u8);
            return m as u8;
        }
        self.vspill.insert(vkey(sp, abs / GRAN), t);
        PTR_SPILL
    }

    /// The register tag bits (a type + 1, or 0) of the pointer stored at
    /// `abs` of space `sp`.
    #[inline]
    pub fn void_tag(&self, sp: Sp, abs: u64) -> u32 {
        match self.space(sp).ptr_mark(abs) {
            0 | 1 => 0,
            PTR_SPILL => self.spilled(sp, abs / GRAN),
            m => self.vtags[usize::from(m - PTR_TAGGED)].0 + 1,
        }
    }

    /// The spilled tag of granule `g` of `sp` (a type + 1, or 0).
    fn spilled(&self, sp: Sp, g: u64) -> u32 {
        self.vspill.get(&vkey(sp, g)).map_or(0, |t| t.0 + 1)
    }

    /// Copies the spilled tags among `n` granules from granule `sg` of
    /// `ssp` to granule `dg` of `dsp` (different spaces).
    fn copy_spilled(&mut self, dsp: Sp, dg: u64, ssp: Sp, sg: u64, n: u64) {
        for i in 0..n {
            if self.space(ssp).ptr[(sg + i) as usize] == PTR_SPILL
                && let Some(&t) = self.vspill.get(&vkey(ssp, sg + i))
            {
                self.vspill.insert(vkey(dsp, dg + i), t);
            }
        }
    }

    /// Heap bytes held (see `Oven::footprint`).
    pub fn footprint(&self) -> usize {
        let space = |s: &Space| s.bytes.capacity() + s.init.capacity() + s.ptr.capacity();
        space(&self.frame)
            + space(&self.heap)
            + space(&self.stat)
            + (self.slots.capacity() + self.statics.capacity() + self.hslots.capacity())
                * size_of::<Alloc>()
            + (self.free.capacity()
                + self.live.capacity()
                + self.hfree.capacity()
                + self.sfree.capacity())
                * 4
            + self.sholes.capacity() * size_of::<(u64, u64)>()
            + self.bins.footprint()
            + self.unions.capacity() * (size_of::<(UKey, u32)>() + 1)
            + self.union_alts.capacity() * (size_of::<(UKey, u64)>() + 1)
            + self.vtags.capacity() * size_of::<OTy>()
            + self.vtag_ix.capacity() * (size_of::<(OTy, u8)>() + 1)
            + self.vspill.capacity() * (size_of::<(u64, OTy)>() + 1)
            + self.trial.footprint()
    }

    /// Folds the running evaluation's [`Memory::arena_peak`] into the
    /// translation unit's ([`Memory::heap_peak`]). Called when an
    /// evaluation context ends (or a new outermost one starts), before
    /// its peak is reset or restored.
    pub fn fold_peak(&mut self) {
        self.tu_arena_peak = self.tu_arena_peak.max(self.arena_peak);
    }

    /// The translation unit's evaluation-memory high-water mark: the most
    /// address space (`frame.top + heap.top` and the reflection bytes, as
    /// `peak_arena` counts it) any evaluation held at once, nested
    /// evaluations included. Memory freed during an evaluation, or
    /// dropped at its end, still counts: this is the peak, exact and
    /// independent of how the buffers grew (`oven.heap-peak`,
    /// spec/oven.md "Memory report"; WP-1315).
    pub fn heap_peak(&self) -> u64 {
        self.tu_arena_peak.max(self.arena_peak)
    }

    /// Drops every frame and heap allocation (end of a top-level
    /// evaluation); buffers are kept.
    pub fn reset(&mut self) {
        self.fold_peak();
        self.frame.top = 0;
        self.life_frame = false;
        self.life_hi = 0;
        self.unconstructed.clear();
        self.slots.truncate(1);
        self.free.clear();
        self.live.clear();
        self.heap.top = 0;
        self.hslots.clear();
        self.hfree.clear();
        self.bins.clear();
        self.heap_live = 0;
        self.heap_count = 0;
        self.heap_allocs = 0;
        self.ctx = 0;
        if !self.unions.is_empty() {
            self.unions.retain(|&k, _| (k.alloc_key() as u32) & STATIC_BIT != 0);
            self.union_epoch += 1;
        }
        if !self.union_alts.is_empty() {
            self.union_alts.retain(|&k, _| (k.alloc_key() as u32) & STATIC_BIT != 0);
            self.union_epoch += 1;
        }
        self.allocated = 0;
        self.peak = 0;
        self.arena_peak = 0;
        self.refl = 0;
        self.trial.clear();
    }

    /// Frame and heap bytes live now: what of [`Memory::allocated`] is
    /// not yet reclaimed (`reclaimed_bytes`).
    pub fn held(&self) -> u64 {
        self.frame.top + self.heap_live
    }

    /// Live bytes now (`peak_bytes` is their maximum).
    pub fn live_bytes(&self) -> u64 {
        self.frame.top + self.heap_live + self.refl
    }

    /// Address space held now (`peak_arena` is its maximum; the memory
    /// limit bounds it).
    pub fn arena(&self) -> u64 {
        self.frame.top + self.heap.top + self.refl
    }

    /// Charges `n` bytes of reflection values and interned content made
    /// by the running evaluation ([`Memory::refl`]).
    ///
    /// # Errors
    /// [`MemFault::Memory`] when the memory limit is exceeded (the bytes
    /// stay charged: the evaluation fails).
    pub fn charge_refl(&mut self, n: u64) -> Result<(), MemFault> {
        self.refl = self.refl.saturating_add(n);
        self.note_peak();
        if self.arena() > self.limit {
            return Err(MemFault::Memory(self.limit));
        }
        Ok(())
    }

    fn note_peak(&mut self) {
        self.peak = self.peak.max(self.live_bytes());
        self.arena_peak = self.arena_peak.max(self.arena());
    }

    /// A fresh frame serial, made current.
    pub fn next_frame(&mut self) -> u32 {
        self.serial = self.serial.wrapping_add(1).max(1);
        self.cur_frame = self.serial;
        self.serial
    }

    /// A frame mark: live-allocation count and arena top.
    pub fn mark(&self) -> u64 {
        (self.live.len() as u64) << 32 | self.frame.top
    }

    pub fn release(&mut self, mark: u64, types: &Types) {
        let n = (mark >> 32) as usize;
        if n >= self.live.len() {
            return;
        }
        for i in n..self.live.len() {
            let s = self.live[i];
            let a = &mut self.slots[s as usize];
            a.live = false;
            if a.flags & flags::HAS_UNION != 0 {
                let key = u64::from(s) | u64::from(a.generation) << 22;
                for &(o, ut) in types.unions(a.ty) {
                    let mut gone = self.unions.remove(&UKey::new(key, o, ut)).is_some();
                    if !self.union_alts.is_empty() {
                        gone |= self.union_alts.remove(&UKey::new(key, o, ut)).is_some();
                    }
                    if gone {
                        self.union_epoch += 1;
                    }
                }
            }
            if a.generation < GEN_MAX {
                self.free.push_back(s);
            }
        }
        self.live.truncate(n);
        self.life_hi = self.life_hi.min(n);
        self.frame.top = mark & 0xFFFF_FFFF;
    }

    /// Allocates a frame object of `size` bytes.
    pub fn alloc(&mut self, ty: OTy, size: u64, fl: u8, origin: Origin) -> Result<u64, MemFault> {
        let base = self.frame.top;
        let end = base + size.max(1).next_multiple_of(GRAN);
        if end + self.heap.top + self.refl > self.limit || end > u64::from(u32::MAX) {
            return Err(MemFault::Memory(self.limit));
        }
        self.frame.grow(end);
        self.frame.set_init(base, end - base, false);
        self.frame.set_ptr(base, end - base, false);
        self.frame.top = end;
        self.allocated += end - base;
        self.note_peak();
        let a = Alloc {
            base: base as u32,
            size: size as u32,
            generation: 0,
            live: true,
            flags: fl,
            ty,
            origin,
            frame: self.cur_frame,
        };
        let reuse = if self.free.len() > FRAME_QUARANTINE { self.free.pop_front() } else { None };
        let slot = if let Some(s) = reuse {
            let g = self.slots[s as usize].generation + 1;
            self.slots[s as usize] = Alloc { generation: g, ..a };
            s
        } else {
            let s = self.slots.len() as u32;
            if s >= HEAP_BIT {
                return Err(MemFault::Memory(self.limit));
            }
            self.slots.push(a);
            s
        };
        self.live.push(slot);
        let generation = self.slots[slot as usize].generation;
        Ok(Ptr { slot, generation, past: false, off: 0 }.encode())
    }

    /// Allocates a static object (initialized by the caller). Outside a
    /// trial it reuses what dead static objects held (first fit) and a
    /// dead slot (its next generation, so a pointer to the dead object
    /// stays stale); under a checkpoint it only grows the space, so a
    /// static object made since the checkpoint is fresh (its base is at
    /// or above the checkpoint's static top).
    pub fn alloc_static(&mut self, ty: OTy, size: u64, fl: u8, origin: Origin) -> u64 {
        self.alloc_static_in(ty, size, fl, origin, 0)
    }

    /// [`Memory::alloc_static`] with [`Alloc::frame`] `frame` ([`LAX_READ`]).
    pub fn alloc_static_in(
        &mut self,
        ty: OTy,
        size: u64,
        fl: u8,
        origin: Origin,
        frame: u32,
    ) -> u64 {
        let reuse = !self.in_trial();
        let base = self.static_range(size);
        let a = Alloc {
            base: base as u32,
            size: size as u32,
            generation: 0,
            live: true,
            flags: fl,
            ty,
            origin,
            frame,
        };
        self.push_static(a, reuse)
    }

    /// Static bytes for an object of `size` (uninitialized, holding no
    /// pointers): a free range (first fit) outside a trial, else the top
    /// of the space ([`Memory::alloc_static`]).
    fn static_range(&mut self, size: u64) -> u64 {
        let len = size.max(1).next_multiple_of(GRAN);
        let hole =
            if self.in_trial() { None } else { self.sholes.iter().position(|&(_, l)| l >= len) };
        let base = match hole {
            Some(i) => {
                let (b, l) = self.sholes[i];
                if l == len {
                    self.sholes.remove(i);
                } else {
                    self.sholes[i] = (b + len, l - len);
                }
                b
            }
            None => {
                let b = self.stat.top;
                self.stat.grow(b + len);
                self.stat.top = b + len;
                self.stat_peak = self.stat_peak.max(self.stat.top);
                b
            }
        };
        self.stat.set_init(base, len, false);
        self.stat.set_ptr(base, len, false);
        base
    }

    /// A static object no access may reach, of origin
    /// [`Origin::External`] or [`Origin::Unknown`]: an
    /// [`flags::EXTERNAL`] slot with a size (for address arithmetic and
    /// bounds) but no bytes in the static space ([`UNBACKED`] base), so
    /// naming a reference to, or an `extern` array of, any size costs one
    /// slot (spec/oven.md "Unknown references"). [`Memory::check_storage`]
    /// rejects every access before the base is used. A size over 4 GiB is
    /// clamped: pointer offsets are 32 bits.
    pub fn alloc_unbacked(&mut self, ty: OTy, size: u64, fl: u8, origin: Origin) -> u64 {
        debug_assert!(fl & flags::EXTERNAL != 0);
        let a = Alloc {
            base: UNBACKED,
            size: size.min(u64::from(u32::MAX)) as u32,
            generation: 0,
            live: true,
            flags: fl | flags::EXTERNAL,
            ty,
            origin,
            frame: 0,
        };
        let reuse = !self.in_trial();
        self.push_static(a, reuse)
    }

    /// Puts static allocation `a` in a dead slot (`reuse`) or a new one.
    fn push_static(&mut self, a: Alloc, reuse: bool) -> u64 {
        let (idx, generation) = match if reuse { self.sfree.pop() } else { None } {
            Some(i) => {
                let g = self.statics[i as usize].generation + 1;
                self.statics[i as usize] = Alloc { generation: g, ..a };
                (i, g)
            }
            None => {
                self.statics.push(a);
                (self.statics.len() as u32 - 1, 0)
            }
        };
        Ptr { slot: idx | STATIC_BIT, generation, past: false, off: 0 }.encode()
    }

    /// Frees static allocation `p` (a scratch object,
    /// `Oven::has_constant_destruction`): its slot is dead (a pointer to
    /// it is stale once the slot is reused) and its range is free for
    /// [`Memory::alloc_static`]; free ranges at the top of the static
    /// space give it back, so the space held does not grow with the
    /// number of dead static objects, wherever they lie (WP-060,
    /// spec/oven.md "Memory"). A pointer that is not to a live static
    /// object is ignored.
    pub fn free_static(&mut self, p: u64, types: &Types) {
        let pp = Ptr::decode(p);
        if pp.is_fn() || pp.space() != Sp::Static {
            return;
        }
        let idx = pp.slot & !STATIC_BIT;
        let Some(a) = self.statics.get_mut(idx as usize) else { return };
        if !a.live || a.generation != pp.generation {
            return;
        }
        a.live = false;
        let a = *a;
        if a.flags & flags::HAS_UNION != 0 {
            let key = Ptr::alloc_key(p);
            for &(o, ut) in types.unions(a.ty) {
                let a = self.unions.remove(&UKey::new(key, o, ut)).is_some();
                if self.union_alts.remove(&UKey::new(key, o, ut)).is_some() || a {
                    self.union_epoch += 1;
                }
            }
        }
        if a.generation < GEN_MAX {
            self.sfree.push(idx);
        }
        if a.base == UNBACKED {
            return;
        }
        let (base, len) = (u64::from(a.base), u64::from(a.size).max(1).next_multiple_of(GRAN));
        // Insert the range, coalescing with its neighbours.
        let i = self.sholes.partition_point(|&(b, _)| b < base);
        let mut range = (base, len);
        if i < self.sholes.len() && self.sholes[i].0 == base + len {
            range.1 += self.sholes.remove(i).1;
        }
        if i > 0 && self.sholes[i - 1].0 + self.sholes[i - 1].1 == base {
            self.sholes[i - 1].1 += range.1;
        } else {
            self.sholes.insert(i, range);
        }
        // Give back a free range at the top, but never below the top an
        // open checkpoint saw: objects above it are fresh to the trial
        // (their writes are not logged), so the range below must not be
        // handed out again before the checkpoint closes.
        let floor = self.trial.cps.last().map_or(0, |c| c.stat_top);
        if let Some(&(b, l)) = self.sholes.last()
            && b + l == self.stat.top
            && self.stat.top > floor
        {
            if b >= floor {
                self.stat.top = b;
                self.sholes.pop();
            } else {
                self.stat.top = floor;
                if let Some(h) = self.sholes.last_mut() {
                    h.1 = floor - b;
                }
            }
        }
    }

    /// Bytes of the static space held: live static objects plus the free
    /// ranges between them ([`Memory::free_static`]).
    pub fn static_top(&self) -> u64 {
        self.stat.top
    }

    /// The most static space ever held ([`Memory::static_top`]'s
    /// high-water mark): it persists across evaluations, so this is the
    /// translation unit's.
    pub fn static_peak(&self) -> u64 {
        self.stat_peak
    }

    /// The allocation a (non-null, non-function) pointer designates.
    pub fn alloc_of(&self, p: Ptr) -> Result<&Alloc, MemFault> {
        let a = match p.space() {
            Sp::Static => self.statics.get((p.slot & !STATIC_BIT) as usize),
            Sp::Heap => self.hslots.get((p.slot - HEAP_BIT) as usize),
            Sp::Frame => self.slots.get(p.slot as usize),
        }
        .ok_or(MemFault::Stale)?;
        if a.generation != p.generation {
            return Err(MemFault::Stale);
        }
        if !a.live {
            return Err(MemFault::Dead(a.origin));
        }
        Ok(a)
    }

    /// Ends the lifetime of the live frame object `p` designates before
    /// its scope releases it (a parameter object, `Op::EndParam`): its
    /// storage stays allocated, and an access to it is to a dead object.
    pub fn kill(&mut self, p: Ptr) {
        let Some(a) = self.slots.get(p.slot as usize) else { return };
        if p.space() != Sp::Frame || a.generation != p.generation || !a.live {
            return;
        }
        if self.in_trial()
            && let Some(cp) = self.trial.cps.last()
            && !self.fresh(cp, Sp::Frame, a)
        {
            self.trial.log.push(Undo::Kill(p.slot));
        }
        self.slots[p.slot as usize].live = false;
    }

    pub fn alloc_of_mut(&mut self, p: Ptr) -> &mut Alloc {
        match p.space() {
            Sp::Static => &mut self.statics[(p.slot & !STATIC_BIT) as usize],
            Sp::Heap => &mut self.hslots[(p.slot - HEAP_BIT) as usize],
            Sp::Frame => &mut self.slots[p.slot as usize],
        }
    }

    /// The allocation now in static slot `slot`, whatever its generation
    /// (for a pointer that points at nothing, [`NOWHERE`]).
    pub fn static_in_slot(&self, slot: u32) -> Option<&Alloc> {
        if slot & STATIC_BIT == 0 || slot >= LABEL_DIFF_SLOT {
            return None;
        }
        self.statics.get((slot & !STATIC_BIT) as usize)
    }

    /// The live or dead (same generation) allocation `p` designates.
    /// Whether leaving a scope at frame mark `mark` may release a
    /// [`flags::LIFE`]-tracked object ([`Memory::life_hi`]); the callers
    /// of `Oven::scope_life` test it inline.
    #[inline]
    pub fn life_above(&self, mark: u64) -> bool {
        self.life_frame && ((mark >> 32) as usize) < self.life_hi
    }

    pub fn alloc_any(&self, p: Ptr) -> Option<&Alloc> {
        match p.space() {
            _ if p.is_fn() => None,
            Sp::Static => self
                .statics
                .get((p.slot & !STATIC_BIT) as usize)
                .filter(|a| a.generation == p.generation),
            Sp::Heap => self
                .hslots
                .get((p.slot - HEAP_BIT) as usize)
                .filter(|a| a.generation == p.generation),
            Sp::Frame => self
                .slots
                .get(p.slot as usize)
                .filter(|a| a.generation == p.generation && p.slot != 0),
        }
    }

    /// [`Memory::alloc_any`] of raw pointer `raw`, decoding only its slot
    /// and generation.
    #[allow(clippy::inline_always)]
    #[inline(always)]
    pub fn alloc_raw(&self, raw: u64) -> Option<&Alloc> {
        let slot = ((raw >> SLOT_SHIFT) & SLOT_MASK) as u32;
        let a = if slot & STATIC_BIT != 0 {
            if slot == FN_SLOT {
                return None;
            }
            self.statics.get((slot & !STATIC_BIT) as usize)?
        } else if slot & HEAP_BIT != 0 {
            self.hslots.get((slot - HEAP_BIT) as usize)?
        } else if slot != 0 {
            self.slots.get(slot as usize)?
        } else {
            return None;
        };
        (u64::from(a.generation) == raw >> GEN_SHIFT).then_some(a)
    }

    /// Checks an access of `len` bytes at `p` and returns its absolute
    /// address in the pointer's space.
    ///
    /// The access that passes every check takes [`Memory::check_plain`],
    /// inlined into every caller (the `Load` and `Store` arms above all),
    /// and, in [`flags::TRACKED`] storage, [`Memory::live_at`]'s lifetime
    /// test (WP-760: a frame object whose lifetimes an explicit
    /// destruction ended, such as an `inplace_vector`'s, is tracked for
    /// every access to it); any other runs [`Memory::check_slow`].
    // Always inlined: the plain predicate is a few compares, while the
    // call and the `Result` returned through memory cost more than it.
    #[allow(clippy::inline_always)]
    #[inline(always)]
    pub fn check(&self, raw: u64, len: u64, write: bool) -> Result<(Sp, u64, &Alloc), MemFault> {
        match self.check_plain(raw, len, flags::EXTERNAL) {
            Some(r) if r.2.flags & flags::TRACKED == 0 || self.live_at(r.0, r.1, len) => Ok(r),
            _ => self.check_slow(raw, len, write),
        }
    }

    /// Whether every byte of `[abs, abs + len)` in space `sp` is within
    /// an object's lifetime: [`Memory::check`]'s test of tracked storage,
    /// out of line (a `bool` in a register, not the `Result` of
    /// [`Memory::check_slow`]).
    #[inline(never)]
    fn live_at(&self, sp: Sp, abs: u64, len: u64) -> bool {
        self.space(sp).is_live(abs, len)
    }

    /// [`Memory::check`] of an access the plain predicate or the lifetime
    /// test refused: the plain access to tracked storage outside its
    /// lifetime faults; any other runs the checks in their order, then
    /// the lifetime test. Cold for the
    /// callers' code layout: marked otherwise, the `Load` arm grows.
    #[cold]
    #[inline(never)]
    fn check_slow(&self, raw: u64, len: u64, write: bool) -> Result<(Sp, u64, &Alloc), MemFault> {
        let r = match self.check_plain(raw, len, flags::EXTERNAL) {
            Some(r) => r,
            None => self.check_ordered(raw, len, write)?,
        };
        if r.2.flags & flags::TRACKED != 0 && !self.space(r.0).is_live(r.1, len) {
            return Err(MemFault::Lifetime);
        }
        Ok(r)
    }

    /// [`Memory::check`] without the lifetime check of tracked storage:
    /// the checks of a placement new, which begins a lifetime.
    ///
    /// The access that passes every check takes [`Memory::check_plain`],
    /// one predicate inlined into the caller; any other runs the checks in
    /// their order ([`Memory::check_ordered`]), so the fault an access
    /// reports is the one the order picks (spec/oven.md "Checks on every
    /// access").
    #[inline]
    pub fn check_storage(
        &self,
        raw: u64,
        len: u64,
        write: bool,
    ) -> Result<(Sp, u64, &Alloc), MemFault> {
        match self.check_plain(raw, len, flags::EXTERNAL) {
            Some(r) => Ok(r),
            None => self.check_ordered(raw, len, write),
        }
    }

    /// The access of `len` bytes at `raw` when [`Memory::check_ordered`]
    /// would accept it without consulting the allocation's origin: a live
    /// allocation of the pointer's slot and generation (never the dead
    /// frame slot 0, so not null; not a function pointer), not past the
    /// end, the bytes in bounds, not [`UNBACKED`] and no flag of `refuse`
    /// set (which holds [`flags::EXTERNAL`]). `None` otherwise, including
    /// every access the ordered checks accept only by its origin (the lax
    /// read of a [`LAX_READ`] object).
    #[allow(clippy::inline_always)]
    #[inline(always)]
    fn check_plain(&self, raw: u64, len: u64, refuse: u8) -> Option<(Sp, u64, &Alloc)> {
        let slot = ((raw >> SLOT_SHIFT) & SLOT_MASK) as u32;
        let (sp, a) = if slot & STATIC_BIT != 0 {
            if slot == FN_SLOT {
                return None;
            }
            (Sp::Static, self.statics.get((slot & !STATIC_BIT) as usize)?)
        } else if slot & HEAP_BIT != 0 {
            (Sp::Heap, self.hslots.get((slot - HEAP_BIT) as usize)?)
        } else {
            (Sp::Frame, self.slots.get(slot as usize)?)
        };
        // With the past-the-end bit kept, a past-the-end pointer's offset
        // exceeds every size (sizes are below 2^32).
        let off = raw & (OFF_MASK | PAST);
        let ok = u64::from(a.generation) == raw >> GEN_SHIFT
            && a.live
            && a.flags & refuse == 0
            && a.base != UNBACKED
            && off + len <= u64::from(a.size);
        ok.then(|| (sp, u64::from(a.base) + off, a))
    }

    /// The checks of [`Memory::check_storage`] in their order, the first
    /// failing one the fault: null, function pointer, slot and generation,
    /// liveness, past the end, bounds, the origin of an
    /// [`flags::EXTERNAL`] object, [`UNBACKED`].
    #[cold]
    #[inline(never)]
    fn check_ordered(
        &self,
        raw: u64,
        len: u64,
        write: bool,
    ) -> Result<(Sp, u64, &Alloc), MemFault> {
        if raw == 0 {
            return Err(MemFault::Null);
        }
        let p = Ptr::decode(raw);
        if p.is_fn() {
            return Err(MemFault::FnPtr);
        }
        let a = self.alloc_of(p)?;
        if p.past {
            return Err(MemFault::PastEnd);
        }
        if u64::from(p.off) + len > u64::from(a.size) {
            return Err(if u64::from(p.off) >= u64::from(a.size) {
                MemFault::PastEnd
            } else {
                MemFault::OutOfBounds
            });
        }
        if a.flags & flags::EXTERNAL != 0 {
            match a.origin {
                Origin::External(_) if self.lax && !write && a.frame == LAX_READ => {}
                Origin::External(d) => return Err(MemFault::External(d, write)),
                Origin::Unknown(d) => return Err(MemFault::Unknown(d)),
                Origin::Let(t) => return Err(MemFault::Temporary(t)),
                // clang: "read of object '&&l' whose value is not known".
                Origin::Label(e) => return Err(MemFault::Literal(Some(e))),
                _ => {}
            }
        }
        if a.base == UNBACKED {
            // Only External and Unknown objects are unbacked.
            return Err(MemFault::Stale);
        }
        Ok((p.space(), u64::from(a.base) + u64::from(p.off), a))
    }

    pub fn space(&self, sp: Sp) -> &Space {
        match sp {
            Sp::Frame => &self.frame,
            Sp::Heap => &self.heap,
            Sp::Static => &self.stat,
        }
    }

    pub fn space_mut(&mut self, sp: Sp) -> &mut Space {
        match sp {
            Sp::Frame => &mut self.frame,
            Sp::Heap => &mut self.heap,
            Sp::Static => &mut self.stat,
        }
    }

    /// Two distinct spaces, the first mutable.
    fn pair(&mut self, d: Sp, s: Sp) -> (&mut Space, &Space) {
        match (d, s) {
            (Sp::Frame, Sp::Heap) => (&mut self.frame, &self.heap),
            (Sp::Frame, _) => (&mut self.frame, &self.stat),
            (Sp::Heap, Sp::Frame) => (&mut self.heap, &self.frame),
            (Sp::Heap, _) => (&mut self.heap, &self.stat),
            (Sp::Static, Sp::Frame) => (&mut self.stat, &self.frame),
            (Sp::Static, _) => (&mut self.stat, &self.heap),
        }
    }

    /// Copies `len` bytes with their shadow (initialization and pointer
    /// marks) from `src` to `dst` (absolute addresses in their spaces).
    pub fn copy(&mut self, dst_sp: Sp, dst: u64, src_sp: Sp, src: u64, len: u64) {
        if dst_sp == src_sp {
            let sp = self.space_mut(dst_sp);
            sp.bytes.copy_within(src as usize..(src + len) as usize, dst as usize);
            copy_bits_within(&mut sp.init, dst, src, len);
            if src.is_multiple_of(GRAN) && dst.is_multiple_of(GRAN) {
                let n = len.div_ceil(GRAN);
                let range = (src / GRAN) as usize..(src / GRAN + n) as usize;
                if sp.ptr[range].contains(&PTR_SPILL) {
                    // Collected first: an overlapping copy reads every
                    // source entry before writing one.
                    let entries: Vec<(u64, OTy)> = (0..n)
                        .filter_map(|g| {
                            let m = self.space(dst_sp).ptr[(src / GRAN + g) as usize];
                            (m == PTR_SPILL)
                                .then(|| self.vspill.get(&vkey(dst_sp, src / GRAN + g)))
                                .flatten()
                                .map(|&t| (vkey(dst_sp, dst / GRAN + g), t))
                        })
                        .collect();
                    self.vspill.extend(entries);
                }
                let sp = self.space_mut(dst_sp);
                sp.ptr.copy_within(
                    (src / GRAN) as usize..(src / GRAN + n) as usize,
                    (dst / GRAN) as usize,
                );
            }
            return;
        }
        let (d, s) = self.pair(dst_sp, src_sp);
        d.bytes[dst as usize..(dst + len) as usize]
            .copy_from_slice(&s.bytes[src as usize..(src + len) as usize]);
        copy_bits(&mut d.init, dst, &s.init, src, len);
        if src.is_multiple_of(GRAN) && dst.is_multiple_of(GRAN) {
            let n = len.div_ceil(GRAN);
            let (sg, dg) = ((src / GRAN) as usize, (dst / GRAN) as usize);
            d.ptr[dg..dg + n as usize].copy_from_slice(&s.ptr[sg..sg + n as usize]);
            self.copy_spilled(dst_sp, dst / GRAN, src_sp, src / GRAN, n);
        }
    }

    /// Zero-fills `len` bytes and marks them initialized.
    pub fn zero(&mut self, sp: Sp, abs: u64, len: u64) {
        let sp = self.space_mut(sp);
        sp.bytes[abs as usize..(abs + len) as usize].fill(0);
        sp.set_init(abs, len, true);
        sp.set_ptr(abs, len, false);
    }
}

// ---- heap -------------------------------------------------------------------

impl Memory {
    /// Allocates a heap object of `size` bytes (uninitialized) for the
    /// `new` expression or `allocate` call `origin`; `fl` may include
    /// [`flags::ARRAY_NEW`] and [`flags::HAS_UNION`]. Fails when the
    /// address space held would exceed the memory limit.
    pub fn heap_alloc(
        &mut self,
        ty: OTy,
        size: u64,
        fl: u8,
        origin: Origin,
    ) -> Result<u64, MemFault> {
        let n = size.max(1).next_multiple_of(GRAN);
        if n > u64::from(u32::MAX)
            || self.hfree.is_empty() && self.hslots.len() >= HEAP_BIT as usize
        {
            return Err(MemFault::Memory(self.limit));
        }
        if self.in_trial() {
            self.trial_heap()?;
        }
        let base = match self.bins.take(n) {
            Some(b) => b,
            None => {
                let base = self.heap.top;
                let end = base + block_size(n);
                if self.frame.top + end + self.refl > self.limit || end > u64::from(u32::MAX) {
                    return Err(MemFault::Memory(self.limit));
                }
                self.heap.grow(end);
                self.heap.top = end;
                base
            }
        };
        self.heap.set_init(base, n, false);
        self.heap.set_ptr(base, n, false);
        if fl & flags::STD_ALLOC != 0 {
            self.heap.ensure_life();
            self.heap.set_life(base, n, false);
        }
        self.allocated += n;
        self.heap_live += n;
        self.heap_count += 1;
        self.heap_allocs += 1;
        self.heap_ops += 1;
        self.note_peak();
        let a = Alloc {
            base: base as u32,
            size: size as u32,
            generation: 0,
            live: true,
            flags: fl,
            ty,
            origin,
            frame: self.ctx,
        };
        let idx = if let Some(i) = self.hfree.pop_front() {
            if self.in_trial() {
                self.trial.log.push(Undo::HSlot(i, Box::new(self.hslots[i as usize])));
            }
            let g = self.hslots[i as usize].generation + 1;
            self.hslots[i as usize] = Alloc { generation: g, ..a };
            i
        } else {
            self.hslots.push(a);
            (self.hslots.len() - 1) as u32
        };
        let generation = self.hslots[idx as usize].generation;
        Ok(Ptr { slot: idx | HEAP_BIT, generation, past: false, off: 0 }.encode())
    }

    /// Deallocates the heap object `raw` points to by form `kind`. Null is
    /// a no-op (diagnosed for `deallocate`). Checked in clang's order: not
    /// a heap allocation, already deleted, wrong form, not the complete
    /// object (with `pointee`, the operand's static pointee type: a
    /// non-array delete needs a pointer to the allocated object itself,
    /// an array delete or `deallocate` one to its first element).
    pub fn heap_free(
        &mut self,
        raw: u64,
        kind: DynKind,
        pointee: Option<OTy>,
        types: &Types,
    ) -> Result<(), MemFault> {
        let Some(i) = self.heap_check(raw, kind, pointee, types)? else {
            return Ok(());
        };
        if self.in_trial() {
            self.trial_heap()?;
        }
        let a = self.hslots[i];
        self.release_heap(i);
        if a.flags & flags::HAS_UNION != 0 {
            let key = Ptr::alloc_key(raw);
            for &(o, ut) in types.unions(a.ty) {
                self.union_put(UKey::new(key, o, ut), None);
            }
        }
        Ok(())
    }

    /// The checks of [`Mem::heap_free`] without the deallocation: the heap
    /// slot `raw` designates (`None` for null). A delete-expression of a
    /// class with a destructor makes them before running it (clang's
    /// `VisitCXXDeleteExpr`: `CheckDeleteKind`, then `HandleDestruction`).
    pub fn heap_check(
        &self,
        raw: u64,
        kind: DynKind,
        pointee: Option<OTy>,
        types: &Types,
    ) -> Result<Option<usize>, MemFault> {
        if raw == 0 {
            return if kind == DynKind::Std { Err(MemFault::DeallocNull) } else { Ok(None) };
        }
        let p = Ptr::decode(raw);
        if p.is_fn() || !p.is_heap() {
            return Err(MemFault::NotHeap);
        }
        let i = (p.slot - HEAP_BIT) as usize;
        let a = match self.hslots.get(i) {
            Some(a) if a.generation == p.generation && a.live => *a,
            _ => return Err(MemFault::DoubleDelete),
        };
        let alloc = DynKind::of(a.flags);
        if alloc != kind {
            return Err(MemFault::DeleteMismatch { alloc, dealloc: kind });
        }
        if p.off != 0 || p.past {
            return Err(MemFault::Subobject);
        }
        if let Some(t) = pointee {
            let whole = if kind == DynKind::New {
                a.ty == t
            } else {
                matches!(types.kind(a.ty), TyKind::Array { elem, .. } if elem == t)
            };
            if !whole {
                return Err(MemFault::Subobject);
            }
        }
        Ok(Some(i))
    }

    /// Begins (`live`, a placement new: the bytes become uninitialized
    /// for the initialization that follows) or ends (an explicit
    /// destruction) the lifetime of the `len`-byte object at `raw`
    /// (checked). `std::allocator` storage is tracked from its
    /// allocation; a frame or `new` object from the first end of a
    /// lifetime in it (`track_life`). Static objects are not tracked
    /// (spec/oven.md "Transient allocation").
    pub fn set_lifetime(&mut self, raw: u64, len: u64, live: bool) -> Result<(), MemFault> {
        let (sp, abs, a) = self.check_storage(raw, len, true)?;
        let mut tracked = a.flags & flags::TRACKED != 0;
        if !live && !tracked && sp != Sp::Static {
            self.track_life(raw)?;
            tracked = true;
        }
        if !live && !tracked {
            return Ok(());
        }
        self.trial_write(raw, len)?;
        let s = self.space_mut(sp);
        s.set_init(abs, len, false);
        if tracked {
            s.set_life(abs, len, live);
        }
        Ok(())
    }

    /// Marks the `len` bytes at `raw` (a base subobject its derived
    /// class's constructor does not construct) outside their lifetime
    /// and uninitialized, tracking the allocation in any space: unlike
    /// [`Memory::set_lifetime`], a static object under construction (a
    /// `constexpr` variable's) is tracked too, so its freeze finds the
    /// base (WP-390). Only reached after Sema diagnosed the constructor.
    /// Its destruction is checked as a frame object's is (`life_frame`).
    /// Entries a rolled-back trial made stay (an error path only).
    pub fn set_unconstructed(&mut self, raw: u64, len: u64, ty: OTy) -> Result<(), MemFault> {
        let (sp, abs, a) = self.check_storage(raw, len, true)?;
        if len == 0 {
            // An empty base: no bytes to mark; the allocation is tracked
            // so its destructions and member calls consult the entry.
            if a.flags & flags::TRACKED == 0 {
                self.track_life(raw)?;
            }
            self.life_frame = true;
            self.unconstructed.push((raw, ty, true));
            return Ok(());
        }
        if a.flags & flags::TRACKED == 0 {
            self.track_life(raw)?;
        }
        self.trial_write(raw, len)?;
        let s = self.space_mut(sp);
        s.set_init(abs, len, false);
        s.set_life(abs, len, false);
        self.life_frame = true;
        self.unconstructed.push((raw, ty, false));
        Ok(())
    }

    /// Whether the `ty` object at `raw` (`len` bytes) contains an
    /// unconstructed base subobject other than itself: its dead bytes
    /// may be that base's alone, the object still within its lifetime.
    pub fn holds_unconstructed(&self, raw: u64, len: u64, ty: OTy) -> bool {
        let p = Ptr::decode(raw);
        let off = u64::from(p.off);
        self.unconstructed.iter().any(|&(b, bt, empty)| {
            let q = Ptr::decode(b);
            let o = u64::from(q.off);
            !empty
                && q.slot == p.slot
                && q.generation == p.generation
                && (off..off + len).contains(&o)
                && (o != off || bt != ty)
        })
    }

    /// Whether the `ty` object at `raw` is an empty base subobject its
    /// derived class's constructor did not construct (WP-401): it has no
    /// bytes whose lifetime could tell.
    pub fn empty_unconstructed(&self, raw: u64, ty: OTy) -> bool {
        let p = Ptr::decode(raw);
        self.unconstructed.iter().any(|&(b, bt, empty)| {
            let q = Ptr::decode(b);
            empty && bt == ty && q.slot == p.slot && q.generation == p.generation && q.off == p.off
        })
    }

    /// Starts tracking the lifetime of the bytes of the (frame or heap)
    /// allocation holding `raw`, all within their lifetime so far. O(size
    /// of the allocation), once per allocation; the bitmask is the
    /// space's, so no memory is held beyond it.
    fn track_life(&mut self, raw: u64) -> Result<(), MemFault> {
        let p = Ptr::decode(raw);
        let a = *self.alloc_of(p)?;
        let base_raw = Ptr { off: 0, past: false, ..p }.encode();
        self.trial_write(base_raw, u64::from(a.size))?;
        self.set_flags(p, a.flags | flags::LIFE);
        let sp = p.space();
        if sp == Sp::Frame {
            self.life_frame = true;
            let i = self.live.iter().rposition(|&s| s == p.slot).map_or(self.live.len(), |i| i + 1);
            self.life_hi = self.life_hi.max(i);
        }
        let s = self.space_mut(sp);
        s.ensure_life();
        s.set_life(u64::from(a.base), u64::from(a.size), true);
        Ok(())
    }

    /// Whether no byte of the `len`-byte object at `raw` is within its
    /// lifetime, in [`flags::TRACKED`] storage (an object an explicit
    /// destruction ended and nothing re-created).
    pub fn ended(&self, raw: u64, len: u64) -> bool {
        let Ok((sp, abs, a)) = self.check_storage(raw, len.max(1), false) else { return false };
        if a.flags & flags::TRACKED == 0 {
            return false;
        }
        let s = self.space(sp);
        (abs..abs + len.max(1)).all(|i| !s.is_live(i, 1))
    }

    /// Re-begins the lifetime of the `len` bytes at `raw` in
    /// [`flags::LIFE`]-tracked storage: a union member made active by
    /// assignment ([class.union.general]/6) after another member's
    /// destruction.
    pub fn revive(&mut self, raw: u64, len: u64) -> Result<(), MemFault> {
        let (sp, abs, a) = self.check_storage(raw, len, true)?;
        if a.flags & flags::LIFE == 0 || self.space(sp).is_live(abs, len) {
            return Ok(());
        }
        self.trial_write(raw, len)?;
        self.space_mut(sp).set_life(abs, len, true);
        Ok(())
    }

    /// Frees heap slot `i` (live) and its block.
    fn release_heap(&mut self, i: usize) {
        if self.in_trial() {
            let a = self.hslots[i];
            self.trial.log.push(Undo::HSlot(i as u32, Box::new(a)));
            // A block that outlives the trial: its bytes are restored even
            // when a later allocation of the trial reuses it.
            if let Some(cp) = self.trial.cps.last().copied()
                && !self.fresh(&cp, Sp::Heap, &a)
            {
                let n = u64::from(a.size).max(1).next_multiple_of(GRAN);
                self.log_range(Sp::Heap, u64::from(a.base), n, cp.serial);
            }
        }
        let a = &mut self.hslots[i];
        a.live = false;
        let (base, n) = (u64::from(a.base), u64::from(a.size).max(1).next_multiple_of(GRAN));
        if a.generation < GEN_MAX {
            self.hfree.push_back(i as u32);
        }
        self.heap_live -= n;
        self.heap_count -= 1;
        self.heap_ops += 1;
        self.bins.give(base, n, &mut self.heap.top);
    }

    /// The live heap allocations of evaluation context `ctx`, as pointers
    /// (the leak check at the end of an evaluation, [expr.const]/5.20).
    pub fn heap_live_of(&self, ctx: u32) -> impl Iterator<Item = (u64, &Alloc)> {
        self.hslots.iter().enumerate().filter(move |(_, a)| a.live && a.frame == ctx).map(
            |(i, a)| {
                let slot = i as u32 | HEAP_BIT;
                (Ptr { slot, generation: a.generation, past: false, off: 0 }.encode(), a)
            },
        )
    }

    /// Frees every live heap allocation of context `ctx` (an evaluation
    /// that ended, diagnosed or not, inside an enclosing one).
    pub fn free_heap_of(&mut self, ctx: u32, types: &Types) {
        if self.heap_count == 0 {
            return;
        }
        for i in 0..self.hslots.len() {
            let a = self.hslots[i];
            if a.live && a.frame == ctx {
                let slot = i as u32 | HEAP_BIT;
                let raw = Ptr { slot, generation: a.generation, past: false, off: 0 }.encode();
                let _ = self.heap_free(raw, DynKind::of(a.flags), None, types);
            }
        }
    }
}

// ---- trial evaluation ---------------------------------------------------------

/// What a trial allows (spec/oven.md "Trial evaluation").
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum TrialMode {
    /// clang's speculative evaluation (`__builtin_constant_p`): modifying
    /// a frame or static object that existed at the checkpoint, or a heap
    /// allocation or deallocation in the checkpoint's evaluation context,
    /// fails with [`MemFault::Speculative`]. Heap objects that existed at
    /// the checkpoint may be modified (and are restored by a rollback).
    Speculative,
    /// Every effect is allowed and undone by a rollback
    /// (`std::meta::substitute` and `can_substitute`, WP-482).
    Transactional,
}

/// An open checkpoint: the state a rollback restores.
#[derive(Clone, Copy, Debug)]
struct Checkpoint {
    mode: TrialMode,
    /// Unique among the checkpoints of the evaluation (`Trial::seen`).
    serial: u32,
    /// Undo log length at the checkpoint.
    pos: usize,
    /// Frame mark ([`Memory::mark`]); its low half is the frame top.
    mark: u64,
    stat_top: u64,
    ctx: u32,
    /// [`Memory::heap_allocs`]: heap objects of `ctx` numbered from it
    /// (`Origin::Heap`) were allocated after the checkpoint.
    heap_allocs: u64,
    /// The heap structure was saved ([`Undo::HeapMeta`]) since.
    heap_saved: bool,
}

/// One undo log entry: an old value, restored by a rollback.
#[derive(Debug)]
enum Undo {
    /// A granule of an object that existed at the innermost checkpoint:
    /// bytes and shadow.
    /// `vtag`: its [`Memory::vspill`] entry when `ptr` is [`PTR_SPILL`]
    /// (a type + 1, else 0).
    Gran { sp: Sp, g: u32, bytes: [u8; 8], init: u8, ptr: u8, life: u8, vtag: u32 },
    /// A union's active-member entry ([`Memory::unions`]).
    Union(UKey, Option<u32>),
    /// A union's alternative active members ([`Memory::union_alts`]).
    UnionAlts(UKey, Option<u64>),
    /// An allocation's flags.
    Flags { sp: Sp, idx: u32, old: u8 },
    /// A frame object [`Memory::kill`] ended.
    Kill(u32),
    /// A heap slot (reused, or freed).
    HSlot(u32, Box<Alloc>),
    /// The heap structure before the first heap allocation or
    /// deallocation under a checkpoint.
    HeapMeta(Box<HeapMeta>),
}

#[derive(Debug)]
struct HeapMeta {
    top: u64,
    live: u64,
    count: u32,
    slots: usize,
    bins: Bins,
    hfree: VecDeque<u32>,
}

/// The open checkpoints of the running evaluation and their undo log.
#[derive(Debug, Default)]
struct Trial {
    cps: Vec<Checkpoint>,
    log: Vec<Undo>,
    /// Granules logged (`sp << 62 | granule`) and the checkpoint serial
    /// they were logged under: a granule is logged once per checkpoint.
    seen: FxHashMap<u64, u32>,
    serial: u32,
}

impl Trial {
    fn footprint(&self) -> usize {
        self.cps.capacity() * size_of::<Checkpoint>()
            + self.log.capacity() * size_of::<Undo>()
            + self
                .log
                .iter()
                .map(|u| match u {
                    Undo::HSlot(..) => size_of::<Alloc>(),
                    Undo::HeapMeta(m) => {
                        size_of::<HeapMeta>() + m.bins.footprint() + m.hfree.capacity() * 4
                    }
                    _ => 0,
                })
                .sum::<usize>()
            + self.seen.capacity() * (size_of::<(u64, u32)>() + 1)
    }

    fn clear(&mut self) {
        self.cps.clear();
        self.log.clear();
        self.seen.clear();
    }
}

impl Memory {
    /// Whether a checkpoint is open (the only test on the hot path).
    #[inline]
    pub fn in_trial(&self) -> bool {
        !self.trial.cps.is_empty()
    }

    /// Whether a [`TrialMode::Speculative`] checkpoint is open: the
    /// evaluation's result may be discarded, so it must not instantiate
    /// or mutate (spec/reflection.md §4.3). A Transactional checkpoint
    /// alone (`std::meta::substitute`) is not speculative: the
    /// evaluations under it are real ones; only their memory effects may
    /// be undone (WP-538). O(open checkpoints).
    pub fn in_speculative_trial(&self) -> bool {
        self.trial.cps.iter().any(|c| c.mode == TrialMode::Speculative)
    }

    /// Opens a checkpoint; its effects are kept by [`Memory::commit`] or
    /// undone by [`Memory::rollback`] (innermost first). O(1).
    pub fn checkpoint(&mut self, mode: TrialMode) {
        self.trial.serial = self.trial.serial.wrapping_add(1);
        self.trial.cps.push(Checkpoint {
            mode,
            serial: self.trial.serial,
            pos: self.trial.log.len(),
            mark: self.mark(),
            stat_top: self.stat.top,
            ctx: self.ctx,
            heap_allocs: self.heap_allocs,
            heap_saved: false,
        });
    }

    /// Closes the innermost checkpoint keeping its effects (an enclosing
    /// checkpoint can still undo them). O(1).
    pub fn commit(&mut self) {
        self.trial.cps.pop();
        if self.trial.cps.is_empty() {
            self.trial.clear();
        }
    }

    /// Closes the innermost checkpoint undoing its effects: objects that
    /// existed at it get their values (bytes, shadow, active union
    /// members, flags) back, heap blocks allocated since are freed and
    /// heap objects freed since are live again, and the frame objects
    /// made since are released. O(log entries since the checkpoint).
    /// Cumulative counters (`allocated`, `heap_allocs`) and peaks keep
    /// the work done.
    pub fn rollback(&mut self, types: &Types) {
        let Some(cp) = self.trial.cps.pop() else {
            return;
        };
        while self.trial.log.len() > cp.pos {
            let Some(u) = self.trial.log.pop() else { break };
            match u {
                Undo::Gran { sp, g, bytes, init, ptr, life, vtag } => {
                    if let Some(t) = vtag.checked_sub(1) {
                        self.vspill.insert(vkey(sp, u64::from(g)), OTy(t));
                    }
                    let s = self.space_mut(sp);
                    let i = g as usize;
                    s.bytes[i * GRAN as usize..(i + 1) * GRAN as usize].copy_from_slice(&bytes);
                    s.init[i] = init;
                    s.ptr[i] = ptr;
                    if let Some(l) = s.life.get_mut(i) {
                        *l = life;
                    }
                }
                Undo::Union(key, old) => {
                    self.union_epoch += 1;
                    match old {
                        Some(v) => self.unions.insert(key, v),
                        None => self.unions.remove(&key),
                    };
                }
                Undo::UnionAlts(key, old) => {
                    self.union_epoch += 1;
                    match old {
                        Some(v) => self.union_alts.insert(key, v),
                        None => self.union_alts.remove(&key),
                    };
                }
                Undo::Flags { sp, idx, old } => {
                    let a = match sp {
                        Sp::Static => &mut self.statics[idx as usize],
                        Sp::Heap => &mut self.hslots[idx as usize],
                        Sp::Frame => &mut self.slots[idx as usize],
                    };
                    a.flags = old;
                }
                Undo::Kill(s) => self.slots[s as usize].live = true,
                Undo::HSlot(i, a) => self.hslots[i as usize] = *a,
                Undo::HeapMeta(m) => {
                    let m = *m;
                    self.heap.top = m.top;
                    self.heap_live = m.live;
                    self.heap_count = m.count;
                    self.hslots.truncate(m.slots);
                    self.bins = m.bins;
                    self.hfree = m.hfree;
                }
            }
        }
        self.release(cp.mark, types);
        if self.trial.cps.is_empty() {
            self.trial.clear();
        }
    }

    /// Whether allocation `a` (in space `sp`) was made after `cp`.
    fn fresh(&self, cp: &Checkpoint, sp: Sp, a: &Alloc) -> bool {
        match sp {
            Sp::Frame => u64::from(a.base) >= cp.mark & 0xFFFF_FFFF,
            Sp::Static => u64::from(a.base) >= cp.stat_top,
            Sp::Heap => {
                a.frame > cp.ctx
                    || a.frame == cp.ctx
                        && matches!(a.origin, Origin::Heap(_, n) if u64::from(n) >= cp.heap_allocs)
            }
        }
    }

    /// Prepares a write of `len` bytes at `raw` (checked) under the open
    /// checkpoints: free outside a trial.
    #[inline]
    pub fn trial_write(&mut self, raw: u64, len: u64) -> Result<(), MemFault> {
        if self.in_trial() { self.trial_log(raw, len) } else { Ok(()) }
    }

    #[inline(never)]
    fn trial_log(&mut self, raw: u64, len: u64) -> Result<(), MemFault> {
        let p = Ptr::decode(raw);
        let Some(a) = self.alloc_any(p).copied() else {
            return Ok(());
        };
        let sp = p.space();
        if sp != Sp::Heap
            && let Some(cp) = self.trial.cps.iter().rev().find(|c| c.mode == TrialMode::Speculative)
            && !self.fresh(cp, sp, &a)
        {
            return Err(MemFault::Speculative);
        }
        let Some(cp) = self.trial.cps.last().copied() else {
            return Ok(());
        };
        if !self.fresh(&cp, sp, &a) {
            self.log_range(sp, u64::from(a.base) + u64::from(p.off), len, cp.serial);
        }
        Ok(())
    }

    /// Logs the granules overlapping `len` bytes at `abs` not yet logged
    /// under checkpoint `serial`.
    fn log_range(&mut self, sp: Sp, abs: u64, len: u64, serial: u32) {
        let (g0, g1) = (abs / GRAN, (abs + len.max(1) - 1) / GRAN);
        for g in g0..=g1 {
            if self.trial.seen.insert((sp as u64) << 62 | g, serial) == Some(serial) {
                continue;
            }
            let s = self.space(sp);
            let i = g as usize;
            let mut bytes = [0u8; 8];
            bytes.copy_from_slice(&s.bytes[i * GRAN as usize..(i + 1) * GRAN as usize]);
            let (init, ptr) = (s.init[i], s.ptr[i]);
            let life = s.life.get(i).copied().unwrap_or(0);
            let vtag = if ptr == PTR_SPILL { self.spilled(sp, g) } else { 0 };
            self.trial.log.push(Undo::Gran { sp, g: g as u32, bytes, init, ptr, life, vtag });
        }
    }

    /// Sets (`Some`) or clears the active-member entry `key` of
    /// [`Memory::unions`], logged under a checkpoint.
    pub fn union_put(&mut self, key: UKey, v: Option<u32>) {
        let old = match v {
            Some(v) => self.unions.insert(key, v),
            None => self.unions.remove(&key),
        };
        if old != v {
            self.union_epoch += 1;
            if self.in_trial() {
                self.trial_union(key, Undo::Union(key, old));
            }
        }
        if !self.union_alts.is_empty() {
            self.union_alts_put(key, None);
        }
    }

    /// Sets or clears the alternative active members of union `key`
    /// ([`Memory::union_alts`]), logged under a checkpoint.
    pub fn union_alts_put(&mut self, key: UKey, v: Option<u64>) {
        let old = match v {
            Some(v) => self.union_alts.insert(key, v),
            None => self.union_alts.remove(&key),
        };
        if old != v {
            self.union_epoch += 1;
            if self.in_trial() {
                self.trial_union(key, Undo::UnionAlts(key, old));
            }
        }
    }

    /// Sets the active member of union `key` while thawing a frozen
    /// value (no checkpoint is open then).
    pub fn union_thaw(&mut self, key: UKey, v: u32) {
        if self.unions.insert(key, v) != Some(v) {
            self.union_epoch += 1;
        }
    }

    /// Whether field `idx` is (or may be, [`Memory::union_alts`]) the
    /// active member of union `key`.
    pub fn may_be_active(&self, key: UKey, idx: u32) -> bool {
        if self.active_memo.get() == (key, idx, self.union_epoch) {
            return true;
        }
        let r = self.unions.get(&key) == Some(&idx)
            || idx < 64
                && !self.union_alts.is_empty()
                && self.union_alts.get(&key).is_some_and(|m| m & 1 << idx != 0);
        if r {
            self.active_memo.set((key, idx, self.union_epoch));
        }
        r
    }

    #[inline(never)]
    fn trial_union(&mut self, key: UKey, undo: Undo) {
        let p = Ptr::decode(key.alloc_key() << SLOT_SHIFT);
        let Some(cp) = self.trial.cps.last().copied() else {
            return;
        };
        if self.alloc_any(p).is_none_or(|a| !self.fresh(&cp, p.space(), a)) {
            self.trial.log.push(undo);
        }
    }

    /// Sets the flags of `p`'s allocation, logged under a checkpoint.
    pub fn set_flags(&mut self, p: Ptr, fl: u8) {
        if self.in_trial()
            && let Some(cp) = self.trial.cps.last().copied()
            && let Some(a) = self.alloc_any(p)
            && a.flags != fl
            && !self.fresh(&cp, p.space(), a)
        {
            let idx = match p.space() {
                Sp::Static => p.slot & !STATIC_BIT,
                Sp::Heap => p.slot - HEAP_BIT,
                Sp::Frame => p.slot,
            };
            self.trial.log.push(Undo::Flags { sp: p.space(), idx, old: a.flags });
        }
        self.alloc_of_mut(p).flags = fl;
    }

    /// A heap allocation or deallocation under a checkpoint: refused in a
    /// speculative trial's own context; the heap structure is saved once
    /// per checkpoint (O(free blocks + free slots)).
    #[inline(never)]
    fn trial_heap(&mut self) -> Result<(), MemFault> {
        let ctx = self.ctx;
        if self.trial.cps.iter().any(|c| c.mode == TrialMode::Speculative && c.ctx == ctx) {
            return Err(MemFault::Speculative);
        }
        let Some(cp) = self.trial.cps.last_mut() else {
            return Ok(());
        };
        if !cp.heap_saved {
            cp.heap_saved = true;
            let m = HeapMeta {
                top: self.heap.top,
                live: self.heap_live,
                count: self.heap_count,
                slots: self.hslots.len(),
                bins: self.bins.clone(),
                hfree: self.hfree.clone(),
            };
            self.trial.log.push(Undo::HeapMeta(Box::new(m)));
        }
        Ok(())
    }
}

// ---- heap blocks ------------------------------------------------------------

/// Largest block served by an exact per-size class.
pub(crate) const SMALL_MAX: u64 = 256;
/// Largest block served by a half-step power-of-two class.
pub(crate) const MEDIUM_MAX: u64 = 1 << 20;
/// 32 exact classes (8..=256 bytes) and 24 half-step classes (384, 512,
/// 768, 1024, ..., 786432, 1048576).
const NCLASS: usize = 32 + 24;

/// The class of a granule-rounded request of `n` bytes and its block
/// size; `None` above [`MEDIUM_MAX`] (a large block of exactly `n` bytes).
pub(crate) fn size_class(n: u64) -> Option<(usize, u64)> {
    if n <= SMALL_MAX {
        return Some(((n / GRAN - 1) as usize, n));
    }
    if n > MEDIUM_MAX {
        return None;
    }
    // 2^p < n <= 2^(p+1), p >= 8: the half step 3 * 2^(p-1) or 2^(p+1).
    let p = (n - 1).ilog2();
    let half = 3u64 << (p - 1);
    let i = 32 + 2 * (p - 8) as usize;
    Some(if n <= half { (i, half) } else { (i + 1, 1 << (p + 1)) })
}

/// The block size of a granule-rounded request.
pub(crate) fn block_size(n: u64) -> u64 {
    size_class(n).map_or(n, |(_, b)| b)
}

/// The free heap blocks: a LIFO list per class and the coalesced free
/// large regions, sorted by base. Every block lies below the heap top.
#[derive(Debug, Default, Clone)]
struct Bins {
    lists: Vec<Vec<u32>>,
    large: Vec<(u64, u64)>,
}

impl Bins {
    fn footprint(&self) -> usize {
        self.lists.capacity() * size_of::<Vec<u32>>()
            + self.lists.iter().map(|l| l.capacity() * 4).sum::<usize>()
            + self.large.capacity() * 16
    }

    fn clear(&mut self) {
        for l in &mut self.lists {
            l.clear();
        }
        self.large.clear();
    }

    /// A free block for a granule-rounded request of `n` bytes: the last
    /// freed block of its class, else the best-fitting large region (the
    /// smallest, then the lowest), carved from its start.
    fn take(&mut self, n: u64) -> Option<u64> {
        let block = match size_class(n) {
            Some((c, b)) => {
                if let Some(base) = self.lists.get_mut(c).and_then(Vec::pop) {
                    return Some(u64::from(base));
                }
                b
            }
            None => n,
        };
        let mut best: Option<usize> = None;
        for (i, &(_, size)) in self.large.iter().enumerate() {
            if size >= block && best.is_none_or(|j| size < self.large[j].1) {
                best = Some(i);
            }
        }
        let i = best?;
        let (base, size) = self.large[i];
        if size == block {
            self.large.remove(i);
        } else {
            self.large[i] = (base + block, size - block);
        }
        Some(base)
    }

    /// Returns the block of a granule-rounded request of `n` bytes at
    /// `base`. A block ending at `top` lowers it (and then any free large
    /// region ending at the new top); a large block is coalesced with its
    /// free neighbours; any other goes on its class's list.
    fn give(&mut self, base: u64, n: u64, top: &mut u64) {
        let class = size_class(n);
        let block = class.map_or(n, |(_, b)| b);
        if base + block == *top {
            *top = base;
        } else if let Some((c, _)) = class {
            if self.lists.len() < NCLASS {
                self.lists.resize_with(NCLASS, Vec::new);
            }
            self.lists[c].push(base as u32);
            return;
        } else {
            self.insert(base, block);
        }
        while let Some(&(b, s)) = self.large.last()
            && b + s == *top
        {
            *top = b;
            self.large.pop();
        }
    }

    /// Adds a free region, merged with its free neighbours.
    fn insert(&mut self, base: u64, mut size: u64) {
        let i = self.large.partition_point(|r| r.0 < base);
        if i < self.large.len() && base + size == self.large[i].0 {
            size += self.large[i].1;
            self.large.remove(i);
        }
        if i > 0 && self.large[i - 1].0 + self.large[i - 1].1 == base {
            self.large[i - 1].1 += size;
        } else {
            self.large.insert(i, (base, size));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MIB: u64 = 1 << 20;

    fn heap(m: &mut Memory, size: u64) -> u64 {
        m.heap_alloc(OTy(0), size, 0, Origin::Heap(ExprId::new(7), 0)).unwrap()
    }

    fn free(m: &mut Memory, p: u64) {
        m.heap_free(p, DynKind::New, None, &Types::default()).unwrap();
    }

    fn base(m: &Memory, p: u64) -> u64 {
        u64::from(m.alloc_any(Ptr::decode(p)).unwrap().base)
    }

    /// The base of a new allocation of `size` bytes.
    fn at(m: &mut Memory, size: u64) -> u64 {
        let p = heap(m, size);
        base(m, p)
    }

    #[test]
    fn size_classes() {
        assert_eq!(size_class(8), Some((0, 8)));
        assert_eq!(size_class(24), Some((2, 24)));
        assert_eq!(size_class(256), Some((31, 256)));
        assert_eq!(size_class(264), Some((32, 384)));
        assert_eq!(size_class(384), Some((32, 384)));
        assert_eq!(size_class(392), Some((33, 512)));
        assert_eq!(size_class(512), Some((33, 512)));
        assert_eq!(size_class(520), Some((34, 768)));
        assert_eq!(size_class(776), Some((35, 1024)));
        assert_eq!(size_class(MIB - 8), Some((55, MIB)));
        assert_eq!(size_class(MIB), Some((55, MIB)));
        assert_eq!(size_class(MIB + 8), None);
        // Every class is a granule multiple and wastes at most a third.
        let mut n = 8;
        while n <= MIB {
            let (c, b) = size_class(n).unwrap();
            assert!(c < NCLASS && b >= n && b % GRAN == 0 && 3 * (b - n) <= b, "{n}");
            n += 8;
        }
    }

    #[test]
    fn small_blocks_reuse_last_freed() {
        let mut m = Memory::new(256 * MIB);
        let a = heap(&mut m, 20);
        let b = heap(&mut m, 24);
        let c = heap(&mut m, 24);
        let _guard = heap(&mut m, 8);
        assert_eq!((base(&m, a), base(&m, b), base(&m, c)), (0, 24, 48));
        free(&mut m, b);
        free(&mut m, c);
        // LIFO: the last freed 24-byte block comes back first.
        assert_eq!(at(&mut m, 24), 48);
        assert_eq!(at(&mut m, 17), 24);
        // Another size does not take a 24-byte block.
        free(&mut m, a);
        assert_eq!(at(&mut m, 16), 80);
        assert_eq!(m.heap.top, 96);
        assert_eq!(m.heap_live, 24 + 24 + 8 + 16);
    }

    #[test]
    fn medium_blocks_reuse_their_class() {
        let mut m = Memory::new(256 * MIB);
        let a = heap(&mut m, 300); // 384-byte class
        let _guard = heap(&mut m, 8);
        free(&mut m, a);
        let b = heap(&mut m, 380);
        assert_eq!(base(&m, b), 0);
        assert_eq!(m.heap.top, 392);
        // Live bytes count the request, not the block.
        assert_eq!(m.heap_live, 384 + 8);
        let c = heap(&mut m, 4000); // 4096-byte class
        free(&mut m, b);
        let _d = heap(&mut m, 8);
        assert_eq!(m.heap.top, 392 + 4096 + 8);
        free(&mut m, c);
        assert_eq!(at(&mut m, 3073), 392);
    }

    #[test]
    fn large_blocks_carve_best_fit() {
        let mut m = Memory::new(256 * MIB);
        let a = heap(&mut m, 3 * MIB);
        let _g1 = heap(&mut m, 8);
        let b = heap(&mut m, 2 * MIB);
        let _g2 = heap(&mut m, 8);
        free(&mut m, a);
        free(&mut m, b);
        // Best fit: the 2 MiB hole, not the 3 MiB one.
        let c = heap(&mut m, 2 * MIB - 64);
        assert_eq!(base(&m, c), 3 * MIB + 8);
        // A small request with an empty list carves the best-fitting large
        // region: the 64 bytes left of the 2 MiB hole, then the 3 MiB one.
        let d = heap(&mut m, 64);
        assert_eq!(base(&m, d), 5 * MIB - 56);
        let e = heap(&mut m, 64);
        assert_eq!(base(&m, e), 0);
        assert_eq!(m.bins.large, vec![(64, 3 * MIB - 64)]);
        let top = m.heap.top;
        assert_eq!(top, 5 * MIB + 16);
    }

    #[test]
    fn doubling_buffer_coalesces_and_lowers_the_top() {
        let mut m = Memory::new(256 * MIB);
        // Grow a buffer by doubling: allocate the new one, free the old.
        let mut buf = heap(&mut m, 2 * MIB);
        let mut size = 2 * MIB;
        for _ in 0..4 {
            size *= 2;
            let next = heap(&mut m, size);
            free(&mut m, buf);
            buf = next;
        }
        // The freed buffers (2+4+8+16 MiB) coalesced into one region below
        // the last one.
        assert_eq!(base(&m, buf), 30 * MIB);
        assert_eq!(m.bins.large, vec![(0, 30 * MIB)]);
        assert_eq!(m.heap.top, 62 * MIB);
        // A buffer that fits the coalesced region reuses it.
        let small = heap(&mut m, 30 * MIB);
        assert_eq!(base(&m, small), 0);
        assert_eq!(m.bins.large, []);
        free(&mut m, small);
        // Freeing the top block lowers the top through the free region.
        free(&mut m, buf);
        assert_eq!(m.heap.top, 0);
        assert_eq!(m.bins.large, []);
        assert_eq!(m.heap_live, 0);
        assert_eq!(m.arena_peak, 62 * MIB);
        assert_eq!(m.peak, 62 * MIB);
    }

    #[test]
    fn freeing_the_top_block_lowers_the_top() {
        let mut m = Memory::new(256 * MIB);
        let a = heap(&mut m, 2 * MIB);
        let b = heap(&mut m, 40);
        let c = heap(&mut m, 40);
        free(&mut m, a);
        free(&mut m, c);
        assert_eq!(m.heap.top, 2 * MIB + 40);
        // b sits on the top now; freeing it reaches the free region too.
        free(&mut m, b);
        assert_eq!(m.heap.top, 0);
    }

    #[test]
    fn deleted_objects_are_detected() {
        let mut m = Memory::new(256 * MIB);
        let e = ExprId::new(3);
        let p = m.heap_alloc(OTy(0), 16, 0, Origin::Heap(e, 0)).unwrap();
        assert!(Ptr::decode(p).is_heap() && Ptr::decode(p).space() == Sp::Heap);
        assert!(m.check(p, 8, false).is_ok());
        free(&mut m, p);
        assert_eq!(m.check(p, 8, false).err(), Some(MemFault::Dead(Origin::Heap(e, 0))));
        assert_eq!(
            m.heap_free(p, DynKind::New, None, &Types::default()),
            Err(MemFault::DoubleDelete)
        );
        // The slot is reused with a new generation: the old pointer is stale.
        let q = heap(&mut m, 16);
        assert_eq!(Ptr::decode(q).slot, Ptr::decode(p).slot);
        assert_ne!(q, p);
        assert_eq!(m.check(p, 8, false).err(), Some(MemFault::Stale));
        assert_eq!(
            m.heap_free(p, DynKind::New, None, &Types::default()),
            Err(MemFault::DoubleDelete)
        );
        assert!(m.check(q, 16, true).is_ok());
    }

    #[test]
    fn delete_faults() {
        let mut m = Memory::new(256 * MIB);
        let t = Types::default();
        assert_eq!(m.heap_free(0, DynKind::ArrayNew, None, &t), Ok(()));
        let f = m.alloc(OTy(0), 8, 0, Origin::None).unwrap();
        assert_eq!(m.heap_free(f, DynKind::New, None, &t), Err(MemFault::NotHeap));
        assert_eq!(m.heap_free(fn_ptr(3), DynKind::New, None, &t), Err(MemFault::NotHeap));
        let s = m.alloc_static(OTy(0), 8, 0, Origin::None);
        assert_eq!(m.heap_free(s, DynKind::New, None, &t), Err(MemFault::NotHeap));
        let a = m.heap_alloc(OTy(0), 32, flags::ARRAY_NEW, Origin::None).unwrap();
        assert_eq!(
            m.heap_free(a, DynKind::New, None, &t),
            Err(MemFault::DeleteMismatch { alloc: DynKind::ArrayNew, dealloc: DynKind::New })
        );
        assert_eq!(
            m.heap_free(Ptr::with_off(a, 8, false), DynKind::ArrayNew, None, &t),
            Err(MemFault::Subobject)
        );
        assert_eq!(
            m.heap_free(Ptr::with_off(a, 32, true), DynKind::ArrayNew, None, &t),
            Err(MemFault::Subobject)
        );
        assert_eq!(m.heap_free(a, DynKind::ArrayNew, None, &t), Ok(()));
        let b = heap(&mut m, 8);
        assert_eq!(
            m.heap_free(b, DynKind::ArrayNew, None, &t),
            Err(MemFault::DeleteMismatch { alloc: DynKind::New, dealloc: DynKind::ArrayNew })
        );
        assert_eq!(m.heap_count, 1);
    }

    #[test]
    fn the_limit_bounds_frame_and_heap_together() {
        let mut m = Memory::new(4096);
        let _f = m.alloc(OTy(0), 1024, 0, Origin::None).unwrap();
        let h = heap(&mut m, 2048);
        assert_eq!(m.alloc(OTy(0), 1032, 0, Origin::None), Err(MemFault::Memory(4096)));
        assert_eq!(m.heap_alloc(OTy(0), 1032, 0, Origin::None), Err(MemFault::Memory(4096)));
        free(&mut m, h);
        assert!(m.alloc(OTy(0), 2048, 0, Origin::None).is_ok());
        assert_eq!(m.arena_peak, 3072);
    }

    #[test]
    fn frame_and_heap_counters() {
        let mut m = Memory::new(256 * MIB);
        let _f = m.alloc(OTy(0), 100, 0, Origin::None).unwrap();
        let a = heap(&mut m, 300);
        assert_eq!(m.allocated, 104 + 304);
        assert_eq!(m.peak, 104 + 304);
        assert_eq!(m.arena_peak, 104 + 384);
        free(&mut m, a);
        let _b = heap(&mut m, 8);
        assert_eq!((m.heap_allocs, m.allocated), (2, 104 + 304 + 8));
        assert_eq!(m.arena(), 104 + 8);
    }

    /// A deterministic generator for the churn tests.
    fn lcg(s: &mut u64) -> u64 {
        *s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        *s >> 33
    }

    #[test]
    fn mixed_churn_fragmentation_is_bounded() {
        let mut m = Memory::new(256 * MIB);
        let sizes = [8, 24, 40, 200];
        let mut live: Vec<u64> = Vec::new();
        let mut seed = 1;
        for i in 0..10_000u32 {
            // Grow towards ~400 live objects, then hover around them.
            let grow = live.len() < 50 || (live.len() < 400 && !lcg(&mut seed).is_multiple_of(3));
            if grow || i.is_multiple_of(2) && live.len() < 400 {
                let s = sizes[(lcg(&mut seed) % 4) as usize];
                live.push(heap(&mut m, s));
            } else {
                let k = (lcg(&mut seed) as usize) % live.len();
                let p = live.swap_remove(k);
                free(&mut m, p);
            }
        }
        assert!(m.peak > 0);
        assert!(m.arena_peak <= 2 * m.peak, "arena {} > 2 * peak {}", m.arena_peak, m.peak);
        for p in live {
            free(&mut m, p);
        }
        assert_eq!(m.heap_live, 0);
    }

    /// spec/oven.md "Memory report" (WP-1315): `heap_peak` is the
    /// translation unit's evaluation-memory high-water mark. It grows with
    /// what is held at once, not with churn; a free or the end of an
    /// evaluation does not lower it; and it is the same in every run.
    #[test]
    fn heap_peak_is_the_tu_high_water_mark() {
        let run = |n: u64, rounds: u32| {
            let mut m = Memory::new(256 * MIB);
            for _ in 0..rounds {
                let ps: Vec<u64> = (0..n).map(|_| heap(&mut m, 1000)).collect();
                for p in ps {
                    free(&mut m, p);
                }
            }
            let during = m.heap_peak();
            assert_eq!(m.heap_live, 0);
            m.reset();
            assert_eq!(m.heap_peak(), during, "reset lowered the peak");
            // A smaller later evaluation leaves it alone.
            let p = heap(&mut m, 8);
            free(&mut m, p);
            m.reset();
            assert_eq!(m.heap_peak(), during);
            during
        };
        let one = run(100, 1);
        assert_eq!(one, 100 * block_size(1000));
        // Ten times the churn at the same peak: the same figure.
        assert_eq!(run(100, 10), one);
        // Ten times as much held at once: ten times the figure.
        assert_eq!(run(1000, 1), 10 * one);
        // Deterministic.
        assert_eq!(run(100, 3), run(100, 3));
        // Proportional where the buffer capacity is not: 5% more held at
        // once is 5% more peak.
        assert_eq!(run(105, 1), 105 * block_size(1000));
    }

    #[test]
    fn footprint_is_flat_over_churn() {
        let mut m = Memory::new(256 * MIB);
        let round = |m: &mut Memory| {
            let mut ps = Vec::new();
            for s in [8, 24, 200, 300, 5000, MIB + 8] {
                ps.push(heap(m, s));
            }
            let f = m.alloc(OTy(0), 64, 0, Origin::None).unwrap();
            let r = m.mark();
            for p in ps.into_iter().rev() {
                free(m, p);
            }
            assert_ne!(f, 0);
            r
        };
        round(&mut m);
        round(&mut m);
        m.reset();
        let warm = m.footprint();
        for i in 0..10_000 {
            round(&mut m);
            if i % 2 == 1 {
                m.reset();
            }
        }
        assert_eq!(m.footprint(), warm);
        // Every round freed everything it allocated on the heap.
        assert_eq!((m.heap.top, m.heap_live, m.heap_count), (0, 0, 0));
    }

    /// Stores a tagged `void *` (tag bits `tag`) at `p` as `Oven::store`
    /// does.
    fn put_tagged(m: &mut Memory, p: u64, tag: u32) {
        let (sp, abs, _) = m.check(p, 8, true).unwrap();
        m.trial_write(p, 8).unwrap();
        let mark = m.set_void_tag(sp, abs, tag);
        let s = m.space_mut(sp);
        s.write(abs, 8, 1);
        s.set_init(abs, 8, true);
        s.set_ptr_mark(abs, 8, mark);
    }

    fn tag_at(m: &Memory, p: u64) -> u32 {
        let (sp, abs, _) = m.check(p, 8, false).unwrap();
        m.void_tag(sp, abs)
    }

    /// spec/oven.md "Construction" (WP-387, WP-395): `void *` tags
    /// intern to stable pointer marks and past the mark space spill to a
    /// per-granule map: no cap on the number of distinct types, and a
    /// spilled tag follows a copy and is restored by a rollback.
    #[test]
    fn void_tags_intern_then_spill() {
        let t = Types::default();
        let mut m = Memory::new(256 * MIB);
        let a = m.alloc(OTy(0), 8 * 400, 0, Origin::None).unwrap();
        let at = |i: u32| Ptr::with_off(a, 8 * u64::from(i), false);
        // Untagged: mark 1, no tag.
        let (sp, abs, _) = m.check(a, 8, true).unwrap();
        assert_eq!(m.set_void_tag(sp, abs, 0), 1);
        for i in 0..400 {
            put_tagged(&mut m, at(i), i + 1);
        }
        assert!((0..400).all(|i| tag_at(&m, at(i)) == i + 1), "no cliff past 254 types");
        let mark = |m: &Memory, i: u64| m.space(sp).ptr_mark(abs + 8 * i);
        assert_eq!((mark(&m, 0), mark(&m, 1), mark(&m, 252)), (PTR_TAGGED, PTR_TAGGED + 1, 254));
        assert_eq!((mark(&m, 253), mark(&m, 399)), (PTR_SPILL, PTR_SPILL));
        // An interned type keeps its mark at any granule.
        put_tagged(&mut m, at(399), 1);
        assert_eq!((mark(&m, 399), tag_at(&m, at(399))), (PTR_TAGGED, 1));
        put_tagged(&mut m, at(399), 400);
        // A granule-aligned copy carries the tags (and an overlapping one
        // reads its sources first).
        let (sp, abs, _) = m.check(a, 8, false).unwrap();
        m.copy(sp, abs + 8, sp, abs, 16);
        assert_eq!((tag_at(&m, at(0)), tag_at(&m, at(1)), tag_at(&m, at(2))), (1, 1, 2));
        let h = heap(&mut m, 16);
        let (hsp, habs, _) = m.check(h, 16, true).unwrap();
        m.copy(hsp, habs, sp, abs + 8 * 300, 16);
        assert_eq!((tag_at(&m, h), tag_at(&m, Ptr::with_off(h, 8, false))), (301, 302));
        // A rollback restores the overwritten tag.
        m.checkpoint(TrialMode::Transactional);
        put_tagged(&mut m, at(299), 450);
        assert_eq!(tag_at(&m, at(299)), 450);
        m.rollback(&t);
        assert_eq!(tag_at(&m, at(299)), 300);
        // A non-pointer write leaves a stale entry the mark hides.
        put(&mut m, at(299), 5).unwrap();
        m.space_mut(sp).set_ptr_mark(abs + 8 * 299, 8, 0);
        assert_eq!(tag_at(&m, at(299)), 0);
    }

    #[test]
    fn reset_clears_the_heap() {
        let mut m = Memory::new(256 * MIB);
        let p = heap(&mut m, 64);
        let q = heap(&mut m, 64);
        free(&mut m, q);
        m.reset();
        assert_eq!((m.heap.top, m.heap_live, m.arena_peak, m.peak), (0, 0, 0, 0));
        assert!(m.hslots.is_empty());
        assert_eq!(m.check(p, 8, false).err(), Some(MemFault::Stale));
        let r = heap(&mut m, 64);
        assert_eq!(base(&m, r), 0);
    }

    #[test]
    fn heap_of_a_context() {
        let mut m = Memory::new(256 * MIB);
        let _outer = heap(&mut m, 16);
        m.ctx = 1;
        let _a = heap(&mut m, 16);
        let _b = m.heap_alloc(OTy(0), 8, flags::ARRAY_NEW, Origin::None).unwrap();
        assert_eq!(m.heap_live_of(1).count(), 2);
        m.free_heap_of(1, &Types::default());
        assert_eq!(m.heap_live_of(1).count(), 0);
        assert_eq!(m.heap_live_of(0).count(), 1);
        assert_eq!(m.heap_count, 1);
    }

    // ---- trials ----

    /// A heap object numbered as `Op::New` numbers them.
    fn hnew(m: &mut Memory, size: u64) -> u64 {
        let n = m.heap_allocs as u32;
        m.heap_alloc(OTy(0), size, 0, Origin::Heap(ExprId::new(7), n)).unwrap()
    }

    fn fnew(m: &mut Memory) -> u64 {
        m.alloc(OTy(0), 8, 0, Origin::None).unwrap()
    }

    /// Stores the 8-byte value `v` at `p` as the VM does.
    fn put(m: &mut Memory, p: u64, v: u64) -> Result<(), MemFault> {
        let (sp, abs, _) = m.check(p, 8, true)?;
        m.trial_write(p, 8)?;
        let s = m.space_mut(sp);
        s.write(abs, 8, u128::from(v));
        s.set_init(abs, 8, true);
        Ok(())
    }

    fn get(m: &Memory, p: u64) -> Option<u64> {
        let (sp, abs, _) = m.check(p, 8, false).ok()?;
        let s = m.space(sp);
        s.is_init(abs, 8).then(|| s.read(abs, 8) as u64)
    }

    #[test]
    fn rollback_restores_interleaved_writes() {
        let t = Types::default();
        let mut m = Memory::new(256 * MIB);
        let x = fnew(&mut m);
        let h = hnew(&mut m, 32);
        let u = fnew(&mut m); // never initialized
        put(&mut m, x, 1).unwrap();
        put(&mut m, h, 2).unwrap();
        let (top, live) = (m.frame.top, m.live.len());
        m.checkpoint(TrialMode::Transactional);
        put(&mut m, x, 10).unwrap();
        let y = fnew(&mut m);
        put(&mut m, y, 5).unwrap();
        put(&mut m, h, 20).unwrap();
        put(&mut m, x, 11).unwrap();
        put(&mut m, u, 3).unwrap();
        put(&mut m, Ptr::with_off(h, 8, false), 21).unwrap();
        assert_eq!((get(&m, x), get(&m, h)), (Some(11), Some(20)));
        m.rollback(&t);
        assert_eq!((get(&m, x), get(&m, h), get(&m, u)), (Some(1), Some(2), None));
        assert_eq!(get(&m, Ptr::with_off(h, 8, false)), None);
        assert!(m.check(y, 8, false).is_err(), "the trial's frame object is released");
        assert_eq!((m.frame.top, m.live.len()), (top, live));
        assert!(!m.in_trial());
        assert!(m.trial.log.is_empty() && m.trial.seen.is_empty());
    }

    #[test]
    fn granules_are_logged_once_per_checkpoint() {
        let mut m = Memory::new(256 * MIB);
        let x = fnew(&mut m);
        m.checkpoint(TrialMode::Transactional);
        for v in 0..100 {
            put(&mut m, x, v).unwrap();
        }
        assert_eq!(m.trial.log.len(), 1);
        m.commit();
        assert_eq!(get(&m, x), Some(99));
        assert!(m.trial.log.is_empty());
    }

    #[test]
    fn rollback_frees_allocations_and_revives_frees() {
        let t = Types::default();
        let mut m = Memory::new(256 * MIB);
        let h1 = hnew(&mut m, 64);
        let h2 = hnew(&mut m, 64);
        put(&mut m, h1, 1).unwrap();
        put(&mut m, h2, 2).unwrap();
        let state = |m: &Memory| (m.heap.top, m.heap_live, m.heap_count, m.hslots.len());
        let before = state(&m);
        let (b1, b2) = (base(&m, h1), base(&m, h2));
        m.checkpoint(TrialMode::Transactional);
        free(&mut m, h1);
        // Reuses h1's block and slot.
        let h3 = hnew(&mut m, 64);
        assert_eq!(base(&m, h3), b1);
        put(&mut m, h3, 30).unwrap();
        let big = hnew(&mut m, 4 * MIB);
        put(&mut m, big, 4).unwrap();
        free(&mut m, h2);
        let h4 = hnew(&mut m, 8);
        free(&mut m, h4);
        assert!(m.arena() > 4 * MIB);
        m.rollback(&t);
        assert_eq!(state(&m), before);
        assert_eq!((get(&m, h1), get(&m, h2)), (Some(1), Some(2)));
        assert!(m.check(h3, 8, false).is_err() && m.check(big, 8, false).is_err());
        assert!(m.arena() < MIB, "rolled-back blocks are released");
        // The heap is consistent: new blocks do not overlap the revived ones.
        let h5 = hnew(&mut m, 64);
        assert!(base(&m, h5) != b1 && base(&m, h5) != b2);
        free(&mut m, h1);
        free(&mut m, h2);
        free(&mut m, h5);
        assert_eq!((m.heap_live, m.heap_count), (0, 0));
        assert_eq!(m.heap.top, 128, "small blocks stay on their lists");
        let h6 = hnew(&mut m, 64);
        assert_eq!(base(&m, h6), b2, "reuses the last freed");
    }

    #[test]
    fn nested_checkpoints() {
        let t = Types::default();
        let mut m = Memory::new(256 * MIB);
        let x = fnew(&mut m);
        let h = hnew(&mut m, 8);
        put(&mut m, x, 0).unwrap();
        m.checkpoint(TrialMode::Transactional);
        put(&mut m, x, 1).unwrap();
        m.checkpoint(TrialMode::Transactional);
        put(&mut m, x, 2).unwrap();
        free(&mut m, h);
        m.rollback(&t);
        assert_eq!(get(&m, x), Some(1));
        assert_eq!(get(&m, h), None, "uninitialized but live again");
        assert!(m.check(h, 8, false).is_ok());
        m.checkpoint(TrialMode::Transactional);
        put(&mut m, x, 3).unwrap();
        put(&mut m, h, 7).unwrap();
        let y = hnew(&mut m, 8);
        m.commit();
        assert_eq!((get(&m, x), get(&m, h)), (Some(3), Some(7)));
        assert!(m.in_trial());
        free(&mut m, y);
        m.rollback(&t);
        assert_eq!((get(&m, x), get(&m, h)), (Some(0), None));
        assert!(m.check(y, 8, false).is_err());
        assert_eq!((m.heap_count, m.hslots.len()), (1, 1));
    }

    #[test]
    fn unions_and_flags_are_restored() {
        let t = Types::default();
        let mut m = Memory::new(256 * MIB);
        let x = fnew(&mut m);
        let key = Ptr::union_key(x, 0, OTy(0));
        m.union_put(key, Some(1));
        m.checkpoint(TrialMode::Transactional);
        m.union_put(key, Some(2));
        m.union_put(key, None);
        m.set_flags(Ptr::decode(x), flags::CONST);
        let y = fnew(&mut m);
        m.union_put(Ptr::union_key(y, 0, OTy(0)), Some(0));
        m.set_flags(Ptr::decode(y), flags::CONST);
        assert_eq!(m.trial.log.len(), 3, "the trial's own object is not logged");
        m.rollback(&t);
        assert_eq!(m.unions.get(&key), Some(&1));
        assert_eq!(m.alloc_of(Ptr::decode(x)).unwrap().flags, 0);
    }

    #[test]
    fn speculative_trials_refuse_outside_effects() {
        let t = Types::default();
        let mut m = Memory::new(256 * MIB);
        let x = fnew(&mut m);
        let h = hnew(&mut m, 8);
        let s = m.alloc_static(OTy(0), 8, 0, Origin::None);
        put(&mut m, h, 1).unwrap();
        m.checkpoint(TrialMode::Speculative);
        assert_eq!(put(&mut m, x, 1), Err(MemFault::Speculative));
        assert_eq!(put(&mut m, s, 1), Err(MemFault::Speculative));
        let own = fnew(&mut m);
        put(&mut m, own, 1).unwrap();
        // Heap objects may be modified.
        put(&mut m, h, 2).unwrap();
        assert_eq!(m.heap_alloc(OTy(0), 8, 0, Origin::None), Err(MemFault::Speculative));
        assert_eq!(m.heap_free(h, DynKind::New, None, &t), Err(MemFault::Speculative));
        // A nested evaluation context allocates and frees its own.
        m.ctx = 1;
        let n = hnew(&mut m, 8);
        put(&mut m, n, 3).unwrap();
        free(&mut m, n);
        m.ctx = 0;
        // A nested speculative trial: objects of the enclosing trial
        // existed before it.
        m.checkpoint(TrialMode::Speculative);
        assert_eq!(put(&mut m, own, 2), Err(MemFault::Speculative));
        m.rollback(&t);
        m.rollback(&t);
        assert_eq!((get(&m, x), get(&m, h)), (None, Some(1)));
        assert_eq!((m.heap_count, m.heap.top), (1, block_size(8)));
        // Committed: heap writes are kept.
        m.checkpoint(TrialMode::Speculative);
        put(&mut m, h, 5).unwrap();
        m.commit();
        assert_eq!(get(&m, h), Some(5));
    }

    #[test]
    fn failed_trials_hold_no_memory() {
        let t = Types::default();
        let mut m = Memory::new(256 * MIB);
        let x = fnew(&mut m);
        let h = hnew(&mut m, 8);
        m.checkpoint(TrialMode::Transactional);
        m.rollback(&t);
        let foot = m.footprint();
        let arena = m.arena_peak;
        for i in 0..1000 {
            m.checkpoint(TrialMode::Transactional);
            put(&mut m, x, i).unwrap();
            put(&mut m, h, i).unwrap();
            let a = hnew(&mut m, 4096);
            put(&mut m, a, i).unwrap();
            let _ = fnew(&mut m);
            m.rollback(&t);
        }
        assert_eq!(m.arena(), arena, "nothing held after the trials");
        assert!(m.arena_peak <= arena + block_size(4096) + GRAN);
        assert!(m.footprint() <= foot + 64 * 1024, "footprint is flat: {}", m.footprint());
        assert_eq!(m.heap_count, 1);
        assert!(m.allocated > 1000 * 4096, "cumulative counters keep the work done");
    }

    fn snew(m: &mut Memory, size: u64) -> u64 {
        m.alloc_static(OTy(0), size, 0, Origin::None)
    }

    /// WP-060, spec/oven.md "Memory": every dead static object is
    /// reclaimed, not only the most recent one, so the static space held
    /// does not grow with the number of dead statics.
    #[test]
    fn dead_statics_are_reclaimed_wherever_they_lie() {
        let t = Types::default();
        let mut m = Memory::new(256 * MIB);
        let keep = snew(&mut m, 8);
        put(&mut m, keep, 7).unwrap();
        // A scratch object followed by a permanent one: the scratch is not
        // the most recent static when it dies.
        let mut perm = Vec::new();
        for i in 0..64 {
            let scratch = snew(&mut m, 256);
            put(&mut m, scratch, i).unwrap();
            let p = snew(&mut m, 8);
            put(&mut m, p, i).unwrap();
            m.free_static(scratch, &t);
            assert!(m.check(scratch, 8, false).is_err(), "dead");
            perm.push(p);
        }
        // 64 permanent 8-byte objects plus at most one scratch hole.
        assert!(m.static_top() <= 8 + 64 * 8 + 256, "{}", m.static_top());
        assert_eq!(get(&m, keep), Some(7));
        for (i, &p) in perm.iter().enumerate() {
            assert_eq!(get(&m, p), Some(i as u64));
        }
        // A reused slot has a new generation: the old pointer is stale,
        // and freeing it again is ignored.
        let a = snew(&mut m, 16);
        m.free_static(a, &t);
        let b = snew(&mut m, 16);
        assert_eq!(Ptr::decode(a).slot, Ptr::decode(b).slot);
        assert_eq!(m.check(a, 8, false).err(), Some(MemFault::Stale));
        m.free_static(a, &t);
        assert!(m.check(b, 8, true).is_ok());
        // Free ranges at the top are given back, coalescing neighbours
        // (sizes past the scratch hole, so these come from the top).
        let top = m.static_top();
        let (x, y) = (snew(&mut m, 520), snew(&mut m, 1000));
        m.free_static(x, &t);
        assert!(m.static_top() > top);
        m.free_static(y, &t);
        assert_eq!(m.static_top(), top);
        // Fresh memory for a new object is uninitialized.
        let z = snew(&mut m, 256);
        assert_eq!(get(&m, z), None);
    }

    /// Statics freed inside a checkpoint are not handed out again before
    /// it closes, and the top never falls below what the checkpoint saw
    /// (objects above it are fresh: their writes are not logged).
    #[test]
    fn dead_statics_in_a_trial() {
        let t = Types::default();
        let mut m = Memory::new(256 * MIB);
        let old = snew(&mut m, 64);
        let top = m.static_top();
        m.checkpoint(TrialMode::Transactional);
        m.free_static(old, &t);
        assert_eq!(m.static_top(), top, "not below the checkpoint's top");
        let n = snew(&mut m, 64);
        assert!(base(&m, n) >= top, "no reuse inside a trial");
        assert_ne!(Ptr::decode(n).slot, Ptr::decode(old).slot);
        m.free_static(n, &t);
        assert_eq!(m.static_top(), top);
        m.commit();
        let r = snew(&mut m, 64);
        assert_eq!(base(&m, r), 0, "reused once the trial closed");
        assert_eq!(m.static_top(), top);
    }

    /// spec/oven.md "Unknown references" (P2280, WP-375): an external or
    /// unknown object holds no bytes whatever its size, every access is
    /// diagnosed, its address arithmetic keeps its bounds, and freeing it
    /// leaves no hole in the static space.
    #[test]
    fn unbacked_statics_hold_no_bytes() {
        let t = Types::default();
        let mut m = Memory::new(256 * MIB);
        let keep = snew(&mut m, 8);
        put(&mut m, keep, 5).unwrap();
        let top = m.static_top();
        let fl = flags::EXTERNAL | flags::CONST;
        let d = DeclId::new(1);
        let u = m.alloc_unbacked(OTy(0), 3_000_000_000, fl, Origin::Unknown(d));
        let x = m.alloc_unbacked(OTy(0), 1 << 40, fl, Origin::External(d));
        assert_eq!(m.static_top(), top, "no bytes");
        assert_eq!(m.check(u, 8, false).err(), Some(MemFault::Unknown(d)));
        let last = Ptr::with_off(u, 2_999_999_992, false);
        assert_eq!(m.check(last, 8, true).err(), Some(MemFault::Unknown(d)));
        assert_eq!(m.check(x, 1, true).err(), Some(MemFault::External(d, true)));
        assert_eq!(m.alloc_any(Ptr::decode(x)).unwrap().size, u32::MAX, "clamped");
        m.free_static(u, &t);
        m.free_static(x, &t);
        assert_eq!(m.static_top(), top);
        let r = snew(&mut m, 8);
        assert_eq!(base(&m, r), top, "no hole was made");
        assert_eq!(get(&m, keep), Some(5));
    }

    // ---- the per-granule bitmasks, against a bit-at-a-time reference ----

    fn bit(v: &[u8], i: u64) -> bool {
        v[(i / GRAN) as usize] >> (i % GRAN) & 1 != 0
    }

    fn set_bit(v: &mut [u8], i: u64, on: bool) {
        let m = 1u8 << (i % GRAN);
        let b = &mut v[(i / GRAN) as usize];
        if on { *b |= m } else { *b &= !m }
    }

    /// The copy of the bits of a range before WP-649, one bit at a time,
    /// every source bit read before any destination bit is written.
    fn copy_bits_ref(d: &mut [u8], dst: u64, s: &[u8], src: u64, len: u64) {
        let bits: Vec<bool> = (0..len).map(|i| bit(s, src + i)).collect();
        for (i, &b) in (0..len).zip(&bits) {
            set_bit(d, dst + i, b);
        }
    }

    /// A map of `n` bytes of pseudo-random bits (xorshift from `seed`).
    fn pattern(n: usize, seed: u64) -> Vec<u8> {
        let mut x = seed | 1;
        (0..n)
            .map(|_| {
                x ^= x << 13;
                x ^= x >> 7;
                x ^= x << 17;
                x as u8
            })
            .collect()
    }

    #[test]
    fn bit_ranges_match_the_bit_at_a_time_reference() {
        // Lengths past `WBITS` take the whole-map-byte paths.
        const LENS: u64 = 140;
        for pos in 0..16 {
            for len in 0..LENS {
                let v = pattern(48, pos * 1000 + len);
                let all = (0..len).all(|i| bit(&v, pos + i));
                assert_eq!(all_bits(&v, pos, len), all, "all_bits {pos} {len}");
                let mut full = v.clone();
                (0..len).for_each(|i| set_bit(&mut full, pos + i, true));
                assert!(all_bits(&full, pos, len));
                for on in [false, true] {
                    let (mut a, mut b) = (v.clone(), v.clone());
                    set_bits(&mut a, pos, len, on);
                    (0..len).for_each(|i| set_bit(&mut b, pos + i, on));
                    assert_eq!(a, b, "set_bits {pos} {len} {on}");
                }
            }
        }
        // A range ending at the map's last byte (the short-word paths).
        let v = pattern(3, 9);
        let mut a = v.clone();
        set_bits(&mut a, 17, 7, true);
        assert_eq!(a, [v[0], v[1], v[2] | 0xFE]);
        assert!(all_bits(&a, 17, 7));
        assert_eq!(all_bits(&a, 16, 8), v[2] & 1 != 0);
    }

    #[test]
    fn copy_moves_init_bits_like_the_reference() {
        let mut m = Memory::new(256 * MIB);
        m.frame.grow(512);
        m.heap.grow(512);
        let (lo, n) = (64u64, 24usize);
        for so in 0..16u64 {
            for d in 0..16u64 {
                for len in 0..140u64 {
                    let seed = so << 16 | d << 8 | len;
                    // Same space, the destination before, at and after the
                    // source (every overlap direction).
                    let (src, dst) = (lo + 16 + so, lo + d * 2);
                    for (src, dst) in [(src, dst), (dst, src)] {
                        m.frame.init[8..8 + n].copy_from_slice(&pattern(n, seed));
                        let mut want = m.frame.init.clone();
                        let snap = want.clone();
                        copy_bits_ref(&mut want, dst, &snap, src, len);
                        m.copy(Sp::Frame, dst, Sp::Frame, src, len);
                        assert_eq!(m.frame.init, want, "same space {src} -> {dst}, {len}");
                    }
                    // Across spaces.
                    m.frame.init[8..8 + n].copy_from_slice(&pattern(n, seed ^ 0x55));
                    m.heap.init[8..8 + n].copy_from_slice(&pattern(n, seed ^ 0xAA));
                    let mut want = m.heap.init.clone();
                    copy_bits_ref(&mut want, lo + d, &m.frame.init, lo + so, len);
                    m.copy(Sp::Heap, lo + d, Sp::Frame, lo + so, len);
                    assert_eq!(m.heap.init, want, "cross space {so} -> {d}, {len}");
                }
            }
        }
    }

    #[test]
    fn read_and_write_match_byte_at_a_time() {
        let mut s = Space::default();
        s.grow(64);
        let end = s.bytes.len() as u64;
        for abs in [0, 5, 8, end - 16, end - 9, end - 3] {
            for len in 0..=16u64.min(end - abs) {
                let v = u128::from_le_bytes(std::array::from_fn(|i| 0xA0 + i as u8));
                let before = pattern(s.bytes.len(), abs * 31 + len);
                s.bytes.copy_from_slice(&before);
                s.write(abs, len, v);
                let mut want = before;
                for i in 0..len as usize {
                    want[abs as usize + i] = 0xA0 + i as u8;
                }
                assert_eq!(s.bytes, want, "write {abs} {len}");
                let r = (0..len)
                    .rev()
                    .fold(0u128, |r, i| r << 8 | u128::from(want[(abs + i) as usize]));
                assert_eq!(s.read(abs, len), r, "read {abs} {len}");
            }
        }
    }

    // ---- the access check, against the checks in their order ----

    /// The body of `Memory::check_storage` before WP-650, kept as the
    /// reference: every check in its order, the first failing one the
    /// fault.
    fn check_reference(
        m: &Memory,
        raw: u64,
        len: u64,
        write: bool,
    ) -> Result<(Sp, u64, &Alloc), MemFault> {
        if raw == 0 {
            return Err(MemFault::Null);
        }
        let p = Ptr::decode(raw);
        if p.is_fn() {
            return Err(MemFault::FnPtr);
        }
        let a = m.alloc_of(p)?;
        if p.past {
            return Err(MemFault::PastEnd);
        }
        if u64::from(p.off) + len > u64::from(a.size) {
            return Err(if u64::from(p.off) >= u64::from(a.size) {
                MemFault::PastEnd
            } else {
                MemFault::OutOfBounds
            });
        }
        if a.flags & flags::EXTERNAL != 0 {
            match a.origin {
                Origin::External(_) if m.lax && !write && a.frame == LAX_READ => {}
                Origin::External(d) => return Err(MemFault::External(d, write)),
                Origin::Unknown(d) => return Err(MemFault::Unknown(d)),
                Origin::Let(t) => return Err(MemFault::Temporary(t)),
                Origin::Label(e) => return Err(MemFault::Literal(Some(e))),
                _ => {}
            }
        }
        if a.base == UNBACKED {
            return Err(MemFault::Stale);
        }
        Ok((p.space(), u64::from(a.base) + u64::from(p.off), a))
    }

    type Checked = Result<(Sp, u64, *const Alloc), MemFault>;

    fn ident(r: Result<(Sp, u64, &Alloc), MemFault>) -> Checked {
        r.map(|(sp, abs, a)| (sp, abs, std::ptr::from_ref(a)))
    }

    /// spec/oven.md "Checks on every access" (WP-650): the plain-access
    /// predicate with the ordered checks behind it gives every access the
    /// result (the fault, or the space, address and allocation) of the
    /// checks run in their order: null, function, stale and bad slots,
    /// dead and reused frame slots, past-the-end pointers, offsets at,
    /// past and far past the end, lengths that overflow the object,
    /// external, unknown, temporary, label and lax-readable objects,
    /// unbacked ones, and tracked storage outside its lifetime.
    #[test]
    fn plain_check_agrees_with_the_ordered_checks() {
        let t = Types::default();
        let mut m = Memory::new(256 * MIB);
        let d = DeclId::new(3);
        let e = ExprId::new(9);
        let mut ptrs = vec![0, fn_ptr(0), fn_ptr(5), Ptr::with_off(NOWHERE, 5, false)];
        // Frame objects released one after another: once more than the
        // quarantine wait, their slots are reused, so the first ones are
        // stale and the last ones dead.
        for i in 0..24 {
            let mk = m.mark();
            ptrs.push(m.alloc(OTy(0), 8 + i, 0, Origin::Var(d)).unwrap());
            m.release(mk, &t);
        }
        ptrs.push(m.alloc(OTy(0), 24, 0, Origin::Var(d)).unwrap());
        ptrs.push(m.alloc(OTy(0), 1, flags::CONST, Origin::Temp(e)).unwrap());
        ptrs.push(m.alloc(OTy(0), 0, 0, Origin::None).unwrap());
        let life = m.alloc(OTy(0), 16, 0, Origin::Var(d)).unwrap();
        m.set_lifetime(Ptr::with_off(life, 8, false), 8, false).unwrap();
        ptrs.push(life);
        // Heap objects: live, deleted, `std::allocator` storage half
        // constructed.
        ptrs.push(heap(&mut m, 40));
        let gone = heap(&mut m, 16);
        free(&mut m, gone);
        ptrs.push(gone);
        let sa = m.heap_alloc(OTy(0), 32, flags::STD_ALLOC, Origin::Heap(e, 9)).unwrap();
        m.set_lifetime(sa, 8, true).unwrap();
        ptrs.push(sa);
        // Static objects of every origin an access looks at.
        ptrs.push(snew(&mut m, 32));
        ptrs.push(m.alloc_static(OTy(0), 16, flags::CONST, Origin::Global(d)));
        ptrs.push(m.alloc_static(OTy(0), 16, flags::EXTERNAL, Origin::Global(d)));
        ptrs.push(m.alloc_static(OTy(0), 16, flags::EXTERNAL, Origin::External(d)));
        ptrs.push(m.alloc_static_in(OTy(0), 16, flags::EXTERNAL, Origin::External(d), LAX_READ));
        ptrs.push(m.alloc_static(OTy(0), 8, flags::EXTERNAL, Origin::Let(d)));
        ptrs.push(m.alloc_unbacked(OTy(0), 8, flags::EXTERNAL, Origin::Label(e)));
        ptrs.push(m.alloc_unbacked(OTy(0), 64, flags::EXTERNAL, Origin::Unknown(d)));
        ptrs.push(m.alloc_unbacked(OTy(0), 1 << 40, flags::EXTERNAL, Origin::External(d)));
        let scratch = snew(&mut m, 8);
        m.free_static(scratch, &t);
        ptrs.push(scratch);
        // Slots no allocation has, in each space, and a wrong generation.
        for slot in [900, HEAP_BIT + 900, STATIC_BIT + 900, FN_SLOT - 1] {
            ptrs.push(Ptr { slot, generation: 0, past: false, off: 0 }.encode());
        }
        let live = *ptrs.last().unwrap();
        ptrs.push(live ^ 1 << GEN_SHIFT);
        let mut seen = std::collections::BTreeSet::new();
        let mut oks = 0;
        for &p in &ptrs {
            let size = m.alloc_any(Ptr::decode(p)).map_or(16, |a| u64::from(a.size));
            for off in [0, 1, 7, 8, size.saturating_sub(1), size, size + 1, u64::from(u32::MAX)] {
                for past in [false, true] {
                    let raw = if p == 0 { 0 } else { Ptr::with_off(p, off, past) };
                    for len in [0, 1, 4, 8, size, size + 1, 1 << 33] {
                        for write in [false, true] {
                            for lax in [false, true] {
                                m.lax = lax;
                                let want = ident(check_reference(&m, raw, len, write));
                                let got = ident(m.check_storage(raw, len, write));
                                assert_eq!(got, want, "check_storage {raw:#x} {len} {write} {lax}");
                                let want = want.and_then(|(sp, abs, a)| {
                                    let fl = m.alloc_any(Ptr::decode(raw)).map_or(0, |a| a.flags);
                                    if fl & flags::TRACKED != 0 && !m.space(sp).is_live(abs, len) {
                                        Err(MemFault::Lifetime)
                                    } else {
                                        Ok((sp, abs, a))
                                    }
                                });
                                let got = ident(m.check(raw, len, write));
                                assert_eq!(got, want, "check {raw:#x} {len} {write} {lax}");
                                match got {
                                    Ok(_) => oks += 1,
                                    Err(f) => {
                                        seen.insert(
                                            format!("{f:?}").split('(').next().unwrap().to_owned(),
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        m.lax = false;
        assert!(oks > 300, "{oks} accesses pass");
        let all = [
            "Null",
            "FnPtr",
            "Dead",
            "Stale",
            "PastEnd",
            "OutOfBounds",
            "External",
            "Unknown",
            "Temporary",
            "Literal",
            "Lifetime",
        ];
        for f in all {
            assert!(seen.contains(f), "no access faults {f}: {seen:?}");
        }
    }
}
