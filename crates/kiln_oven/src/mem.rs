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
//! mark (the granule holds a pointer, whose bytes are an encoded
//! [`Ptr`]). Allocations start on a granule boundary.
//!
//! A pointer is a `u64`: `offset (32) | past-end (1) | slot (22) | gen (9)`.
//! Null is 0 (frame slot 0 is never used). Slot numbers with bit 21 set
//! are static, those with bit 20 (and not 21) set are heap slots; the
//! all-ones slot is a function pointer whose offset is the `FuncId`. A
//! frame or heap slot's generation is bumped when the slot is reused, so a
//! dangling pointer never matches a later allocation in the same slot
//! (until then it designates the dead allocation, whose origin names it in
//! notes); a slot whose generation would wrap is retired.

use std::collections::VecDeque;

use kiln_ast::{ConstValueId, DeclId, ExprId, QualType, StringLitId};
use kiln_source::fx::FxHashMap;

use crate::ty::{OTy, TyKind, Types};

pub(crate) const GRAN: u64 = 8;
const OFF_MASK: u64 = 0xFFFF_FFFF;
pub(crate) const PAST: u64 = 1 << 32;
const SLOT_SHIFT: u32 = 33;
const SLOT_MASK: u64 = (1 << 22) - 1;
const GEN_SHIFT: u32 = 55;
const GEN_MAX: u16 = (1 << 9) - 1;
pub(crate) const STATIC_BIT: u32 = 1 << 21;
/// Marks a heap slot (frame slots are below it).
pub(crate) const HEAP_BIT: u32 = 1 << 20;
pub(crate) const FN_SLOT: u32 = (1 << 22) - 1;

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

    /// The key of the union subobject at byte `off` of `p`'s allocation
    /// in [`Memory::unions`].
    pub fn union_key(p: u64, off: u64) -> u64 {
        Ptr::alloc_key(p) << 32 | off
    }

    /// `p` with its offset replaced.
    pub fn with_off(p: u64, off: u64, past: bool) -> u64 {
        (p & !(OFF_MASK | PAST)) | (off & OFF_MASK) | if past { PAST } else { 0 }
    }
}

/// One of the three byte spaces.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Sp {
    Frame,
    Heap,
    Static,
}

pub(crate) fn fn_ptr(func: u32) -> u64 {
    Ptr { slot: FN_SLOT, generation: 0, past: false, off: func }.encode()
}

/// What an allocation is, for diagnostics and freezing.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
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
    /// A heap allocation made by `std::allocator<T>::allocate`: storage
    /// for `T[n]` whose elements are outside their lifetime until
    /// constructed (`Space::life`, spec/oven.md "Transient allocation").
    pub const STD_ALLOC: u8 = 64;
    /// A frame or `new` object one of whose subobjects' lifetime an
    /// explicit destruction ended: `Space::life` is kept for its bytes
    /// from then on (`Memory::set_lifetime`; spec/oven.md "Object
    /// lifetime outside std::allocator storage").
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

#[derive(Clone, Copy, Debug)]
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
    /// ([`Memory::ctx`]) that allocated it.
    pub frame: u32,
}

#[derive(Debug, Default)]
pub(crate) struct Space {
    pub bytes: Vec<u8>,
    /// Per granule: bit i = byte i is initialized.
    pub init: Vec<u8>,
    /// Per granule: holds a pointer.
    pub ptr: Vec<u8>,
    /// Per granule: bit i = byte i is within the lifetime of an object.
    /// Meaningful for [`flags::TRACKED`] allocations only, and empty
    /// until the first one (spec/oven.md "Transient allocation").
    pub life: Vec<u8>,
    pub top: u64,
}

/// Whether every byte of `[abs, abs + len)` has its bit set in the
/// per-granule bitmask `v`.
fn all_bits(v: &[u8], abs: u64, len: u64) -> bool {
    let (mut a, end) = (abs, abs + len);
    while a < end {
        let g = (a / GRAN) as usize;
        let lo = (a % GRAN) as u32;
        let hi = ((end - a + u64::from(lo)).min(GRAN)) as u32;
        let mask = (((1u32 << hi) - 1) & !((1u32 << lo) - 1)) as u8;
        if v[g] & mask != mask {
            return false;
        }
        a += u64::from(hi - lo);
    }
    true
}

/// Sets (`on`) or clears the bits of `[abs, abs + len)` in the
/// per-granule bitmask `v`.
fn set_bits(v: &mut [u8], abs: u64, len: u64, on: bool) {
    if len == 0 {
        return;
    }
    let end = abs + len;
    let (g0, g1) = (abs / GRAN, (end - 1) / GRAN);
    let mask = |lo: u64, hi: u64| (((1u32 << hi) - 1) & !((1u32 << lo) - 1)) as u8;
    let mut put = |g: u64, m: u8| {
        let b = &mut v[g as usize];
        if on { *b |= m } else { *b &= !m }
    };
    if g0 == g1 {
        put(g0, mask(abs % GRAN, end - g0 * GRAN));
        return;
    }
    put(g0, mask(abs % GRAN, GRAN));
    put(g1, mask(0, end - g1 * GRAN));
    v[(g0 + 1) as usize..g1 as usize].fill(if on { 0xFF } else { 0 });
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

    pub fn read(&self, abs: u64, len: u64) -> u128 {
        let mut v = 0u128;
        for i in (0..len).rev() {
            v = v << 8 | u128::from(self.bytes[(abs + i) as usize]);
        }
        v
    }

    pub fn write(&mut self, abs: u64, len: u64, mut v: u128) {
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

#[derive(Debug)]
pub(crate) struct Memory {
    pub frame: Space,
    pub heap: Space,
    pub stat: Space,
    pub slots: Vec<Alloc>,
    /// Dead slots, reused first-in first-out so a dangling pointer stays
    /// diagnosable (its slot keeps its origin) for as long as possible.
    free: VecDeque<u32>,
    pub live: Vec<u32>,
    pub statics: Vec<Alloc>,
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
    /// Active member of each union subobject with one: key
    /// `alloc_key << 32 | offset`, value the field index.
    pub unions: FxHashMap<u64, u32>,
    /// Bytes allocated (cumulative, granule-rounded; frame and heap).
    pub allocated: u64,
    /// Peak live bytes: `frame.top + heap_live`.
    pub peak: u64,
    /// Peak address space held: `frame.top + heap.top`.
    pub arena_peak: u64,
    pub limit: u64,
    /// Serial of the running call frame, and the last serial handed out.
    pub cur_frame: u32,
    pub serial: u32,
    /// A frame object has been [`flags::LIFE`]-tracked in this evaluation:
    /// leaving a scope checks the objects it releases (`Oven::scope_life`).
    pub life_frame: bool,
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
            hslots: Vec::new(),
            hfree: VecDeque::new(),
            bins: Bins::default(),
            heap_live: 0,
            heap_count: 0,
            heap_allocs: 0,
            heap_ops: 0,
            ctx: 0,
            unions: FxHashMap::default(),
            allocated: 0,
            peak: 0,
            arena_peak: 0,
            limit,
            cur_frame: 0,
            serial: 0,
            life_frame: false,
            trial: Trial::default(),
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
            + (self.free.capacity() + self.live.capacity() + self.hfree.capacity()) * 4
            + self.bins.footprint()
            + self.unions.capacity() * (size_of::<(u64, u32)>() + 1)
            + self.trial.footprint()
    }

    /// Drops every frame and heap allocation (end of a top-level
    /// evaluation); buffers are kept.
    pub fn reset(&mut self) {
        self.frame.top = 0;
        self.life_frame = false;
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
            self.unions.retain(|&k, _| ((k >> 32) as u32) & STATIC_BIT != 0);
        }
        self.allocated = 0;
        self.peak = 0;
        self.arena_peak = 0;
        self.trial.clear();
    }

    /// Live bytes now (`peak_bytes` is their maximum).
    pub fn live_bytes(&self) -> u64 {
        self.frame.top + self.heap_live
    }

    /// Address space held now (`peak_arena` is its maximum; the memory
    /// limit bounds it).
    pub fn arena(&self) -> u64 {
        self.frame.top + self.heap.top
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
                for &(o, _) in types.unions(a.ty) {
                    self.unions.remove(&(key << 32 | o));
                }
            }
            if a.generation < GEN_MAX {
                self.free.push_back(s);
            }
        }
        self.live.truncate(n);
        self.frame.top = mark & 0xFFFF_FFFF;
    }

    /// Allocates a frame object of `size` bytes.
    pub fn alloc(&mut self, ty: OTy, size: u64, fl: u8, origin: Origin) -> Result<u64, MemFault> {
        let base = self.frame.top;
        let end = base + size.max(1).next_multiple_of(GRAN);
        if end + self.heap.top > self.limit || end > u64::from(u32::MAX) {
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
        let slot = if let Some(s) = self.free.pop_front() {
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

    /// Allocates a static object (initialized by the caller).
    pub fn alloc_static(&mut self, ty: OTy, size: u64, fl: u8, origin: Origin) -> u64 {
        let base = self.stat.top;
        let end = base + size.max(1).next_multiple_of(GRAN);
        self.stat.grow(end);
        self.stat.set_init(base, end - base, false);
        self.stat.set_ptr(base, end - base, false);
        self.stat.top = end;
        let idx = self.statics.len() as u32;
        self.statics.push(Alloc {
            base: base as u32,
            size: size as u32,
            generation: 0,
            live: true,
            flags: fl,
            ty,
            origin,
            frame: 0,
        });
        Ptr { slot: idx | STATIC_BIT, generation: 0, past: false, off: 0 }.encode()
    }

    /// The allocation a (non-null, non-function) pointer designates.
    pub fn alloc_of(&self, p: Ptr) -> Result<&Alloc, MemFault> {
        let a = match p.space() {
            Sp::Static => {
                return self.statics.get((p.slot & !STATIC_BIT) as usize).ok_or(MemFault::Stale);
            }
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

    pub fn alloc_of_mut(&mut self, p: Ptr) -> &mut Alloc {
        match p.space() {
            Sp::Static => &mut self.statics[(p.slot & !STATIC_BIT) as usize],
            Sp::Heap => &mut self.hslots[(p.slot - HEAP_BIT) as usize],
            Sp::Frame => &mut self.slots[p.slot as usize],
        }
    }

    /// The live or dead (same generation) allocation `p` designates.
    pub fn alloc_any(&self, p: Ptr) -> Option<&Alloc> {
        match p.space() {
            _ if p.is_fn() => None,
            Sp::Static => self.statics.get((p.slot & !STATIC_BIT) as usize),
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

    /// Checks an access of `len` bytes at `p` and returns its absolute
    /// address in the pointer's space.
    #[inline]
    pub fn check(&self, raw: u64, len: u64, write: bool) -> Result<(Sp, u64, &Alloc), MemFault> {
        let r = self.check_storage(raw, len, write)?;
        if r.2.flags & flags::TRACKED != 0 && !self.space(r.0).is_live(r.1, len) {
            return Err(MemFault::Lifetime);
        }
        Ok(r)
    }

    /// [`Memory::check`] without the lifetime check of tracked storage:
    /// the checks of a placement new, which begins a lifetime.
    pub fn check_storage(
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
                Origin::External(d) => return Err(MemFault::External(d, write)),
                Origin::Let(t) => return Err(MemFault::Temporary(t)),
                _ => {}
            }
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
            for i in 0..len {
                let v = sp.init[((src + i) / GRAN) as usize] >> ((src + i) % GRAN) & 1 != 0;
                sp.set_init(dst + i, 1, v);
            }
            if src.is_multiple_of(GRAN) && dst.is_multiple_of(GRAN) {
                for g in 0..len.div_ceil(GRAN) {
                    sp.ptr[(dst / GRAN + g) as usize] = sp.ptr[(src / GRAN + g) as usize];
                }
            }
            return;
        }
        let (d, s) = self.pair(dst_sp, src_sp);
        d.bytes[dst as usize..(dst + len) as usize]
            .copy_from_slice(&s.bytes[src as usize..(src + len) as usize]);
        for i in 0..len {
            let v = s.init[((src + i) / GRAN) as usize] >> ((src + i) % GRAN) & 1 != 0;
            d.set_init(dst + i, 1, v);
        }
        if src.is_multiple_of(GRAN) && dst.is_multiple_of(GRAN) {
            for g in 0..len.div_ceil(GRAN) {
                d.ptr[(dst / GRAN + g) as usize] = s.ptr[(src / GRAN + g) as usize];
            }
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
                if self.frame.top + end > self.limit || end > u64::from(u32::MAX) {
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
            for &(o, _) in types.unions(a.ty) {
                self.union_put(key << 32 | o, None);
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
    /// Every effect is allowed and undone by a rollback.
    #[cfg_attr(not(test), expect(dead_code, reason = "no evaluator trial needs it yet"))]
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
    Gran { sp: Sp, g: u32, bytes: [u8; 8], init: u8, ptr: u8, life: u8 },
    /// A union's active-member entry ([`Memory::unions`]).
    Union(u64, Option<u32>),
    /// An allocation's flags.
    Flags { sp: Sp, idx: u32, old: u8 },
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
                Undo::Gran { sp, g, bytes, init, ptr, life } => {
                    let s = self.space_mut(sp);
                    let i = g as usize;
                    s.bytes[i * GRAN as usize..(i + 1) * GRAN as usize].copy_from_slice(&bytes);
                    s.init[i] = init;
                    s.ptr[i] = ptr;
                    if let Some(l) = s.life.get_mut(i) {
                        *l = life;
                    }
                }
                Undo::Union(key, old) => match old {
                    Some(v) => {
                        self.unions.insert(key, v);
                    }
                    None => {
                        self.unions.remove(&key);
                    }
                },
                Undo::Flags { sp, idx, old } => {
                    let a = match sp {
                        Sp::Static => &mut self.statics[idx as usize],
                        Sp::Heap => &mut self.hslots[idx as usize],
                        Sp::Frame => &mut self.slots[idx as usize],
                    };
                    a.flags = old;
                }
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
            self.trial.log.push(Undo::Gran { sp, g: g as u32, bytes, init, ptr, life });
        }
    }

    /// Sets (`Some`) or clears the active-member entry `key` of
    /// [`Memory::unions`], logged under a checkpoint.
    pub fn union_put(&mut self, key: u64, v: Option<u32>) {
        let old = match v {
            Some(v) => self.unions.insert(key, v),
            None => self.unions.remove(&key),
        };
        if old != v && self.in_trial() {
            self.trial_union(key, old);
        }
    }

    #[inline(never)]
    fn trial_union(&mut self, key: u64, old: Option<u32>) {
        let p = Ptr::decode((key >> 32) << SLOT_SHIFT);
        let Some(cp) = self.trial.cps.last().copied() else {
            return;
        };
        if self.alloc_any(p).is_none_or(|a| !self.fresh(&cp, p.space(), a)) {
            self.trial.log.push(Undo::Union(key, old));
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
        let key = Ptr::union_key(x, 0);
        m.union_put(key, Some(1));
        m.checkpoint(TrialMode::Transactional);
        m.union_put(key, Some(2));
        m.union_put(key, None);
        m.set_flags(Ptr::decode(x), flags::CONST);
        let y = fnew(&mut m);
        m.union_put(Ptr::union_key(y, 0), Some(0));
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
}
