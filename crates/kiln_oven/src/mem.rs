//! Abstract memory: allocations with byte images plus a shadow.
//!
//! Two spaces hold bytes: the *frame* space (locals and temporaries of
//! the running evaluation, a stack released by scope marks and reset after
//! every top-level evaluation) and the *static* space (thawed `constexpr`
//! variables, string literals and external objects; immutable, kept for
//! the translation unit and shared by later evaluations).
//!
//! Each space keeps, per 8-byte granule, a byte-granular initialization
//! mask (bit `i` = byte `i` of the granule holds a value) and a pointer
//! mark (the granule holds a pointer, whose bytes are an encoded
//! [`Ptr`]). Allocations start on a granule boundary.
//!
//! A pointer is a `u64`: `offset (32) | past-end (1) | slot (22) | gen (9)`.
//! Null is 0 (frame slot 0 is never used). Slot numbers with bit 21 set
//! are static; the all-ones slot is a function pointer whose offset is the
//! `FuncId`. A frame slot's generation is bumped when the slot is reused,
//! so a dangling pointer never matches a later allocation in the same
//! slot (until then it designates the dead allocation, whose origin names
//! it in notes); a slot whose generation would wrap is retired.

use std::collections::VecDeque;

use kiln_ast::{ConstValueId, DeclId, ExprId, StringLitId};
use kiln_source::fx::FxHashMap;

use crate::ty::{OTy, Types};

pub(crate) const GRAN: u64 = 8;
const OFF_MASK: u64 = 0xFFFF_FFFF;
pub(crate) const PAST: u64 = 1 << 32;
const SLOT_SHIFT: u32 = 33;
const SLOT_MASK: u64 = (1 << 22) - 1;
const GEN_SHIFT: u32 = 55;
const GEN_MAX: u16 = (1 << 9) - 1;
pub(crate) const STATIC_BIT: u32 = 1 << 21;
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

    pub fn is_static(self) -> bool {
        self.slot & STATIC_BIT != 0
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
    /// A string literal.
    Str(StringLitId),
    /// An anonymous constant thawed from the store.
    Const(ConstValueId),
}

pub(crate) mod flags {
    pub const CONST: u8 = 1;
    pub const EXTERNAL: u8 = 2;
    pub const HAS_UNION: u8 = 4;
    /// A global whose initializer is being evaluated (a self-reference
    /// reads an object outside its lifetime).
    pub const RESOLVING: u8 = 8;
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
}

#[derive(Debug, Default)]
pub(crate) struct Space {
    pub bytes: Vec<u8>,
    /// Per granule: bit i = byte i is initialized.
    pub init: Vec<u8>,
    /// Per granule: holds a pointer.
    pub ptr: Vec<u8>,
    pub top: u64,
}

impl Space {
    fn grow(&mut self, end: u64) {
        let need = end as usize;
        if self.bytes.len() < need {
            let n = need.max(self.bytes.len() * 2).next_multiple_of(GRAN as usize);
            self.bytes.resize(n, 0);
            self.init.resize(n / GRAN as usize, 0);
            self.ptr.resize(n / GRAN as usize, 0);
        }
    }

    pub fn is_init(&self, abs: u64, len: u64) -> bool {
        let (mut a, end) = (abs, abs + len);
        while a < end {
            let g = (a / GRAN) as usize;
            let lo = (a % GRAN) as u32;
            let hi = ((end - a + u64::from(lo)).min(GRAN)) as u32;
            let mask = (((1u32 << hi) - 1) & !((1u32 << lo) - 1)) as u8;
            if self.init[g] & mask != mask {
                return false;
            }
            a += u64::from(hi - lo);
        }
        true
    }

    pub fn set_init(&mut self, abs: u64, len: u64, v: bool) {
        let (mut a, end) = (abs, abs + len);
        while a < end {
            let g = (a / GRAN) as usize;
            let lo = (a % GRAN) as u32;
            let hi = ((end - a + u64::from(lo)).min(GRAN)) as u32;
            let mask = (((1u32 << hi) - 1) & !((1u32 << lo) - 1)) as u8;
            if v {
                self.init[g] |= mask;
            } else {
                self.init[g] &= !mask;
            }
            a += u64::from(hi - lo);
        }
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
        for g in g0..=g1 {
            self.ptr[g as usize] = u8::from(v);
        }
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
    /// Modification of a namespace-scope or static variable.
    ModifyGlobal,
    /// Read (`false`) or write (`true`) of an external variable.
    External(DeclId, bool),
    /// A pointer read from non-pointer bytes or vice versa.
    Incompatible,
    Memory(u64),
}

#[derive(Debug)]
pub(crate) struct Memory {
    pub frame: Space,
    pub stat: Space,
    pub slots: Vec<Alloc>,
    /// Dead slots, reused first-in first-out so a dangling pointer stays
    /// diagnosable (its slot keeps its origin) for as long as possible.
    free: VecDeque<u32>,
    pub live: Vec<u32>,
    pub statics: Vec<Alloc>,
    /// Active member of each union subobject with one: key
    /// `alloc_key << 32 | offset`, value the field index.
    pub unions: FxHashMap<u64, u32>,
    /// Frame bytes allocated (cumulative, granule-rounded).
    pub allocated: u64,
    /// Peak live frame bytes.
    pub peak: u64,
    pub limit: u64,
}

const DEAD0: Alloc = Alloc {
    base: 0,
    size: 0,
    generation: 0,
    live: false,
    flags: 0,
    ty: OTy(0),
    origin: Origin::None,
};

impl Memory {
    pub fn new(limit: u64) -> Self {
        Memory {
            frame: Space::default(),
            stat: Space::default(),
            slots: vec![DEAD0],
            free: VecDeque::new(),
            live: Vec::new(),
            statics: Vec::new(),
            unions: FxHashMap::default(),
            allocated: 0,
            peak: 0,
            limit,
        }
    }

    /// Drops every frame allocation (end of a top-level evaluation).
    pub fn reset(&mut self) {
        self.frame.top = 0;
        self.slots.truncate(1);
        self.free.clear();
        self.live.clear();
        if !self.unions.is_empty() {
            self.unions.retain(|&k, _| ((k >> 32) as u32) & STATIC_BIT != 0);
        }
        self.allocated = 0;
        self.peak = 0;
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
        if end > self.limit || end > u64::from(u32::MAX) {
            return Err(MemFault::Memory(self.limit));
        }
        self.frame.grow(end);
        self.frame.set_init(base, end - base, false);
        self.frame.set_ptr(base, end - base, false);
        self.frame.top = end;
        self.allocated += end - base;
        self.peak = self.peak.max(end);
        let a = Alloc {
            base: base as u32,
            size: size as u32,
            generation: 0,
            live: true,
            flags: fl,
            ty,
            origin,
        };
        let slot = if let Some(s) = self.free.pop_front() {
            let g = self.slots[s as usize].generation + 1;
            self.slots[s as usize] = Alloc { generation: g, ..a };
            s
        } else {
            let s = self.slots.len() as u32;
            if s >= STATIC_BIT {
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
        });
        Ptr { slot: idx | STATIC_BIT, generation: 0, past: false, off: 0 }.encode()
    }

    /// The allocation a (non-null, non-function) pointer designates.
    pub fn alloc_of(&self, p: Ptr) -> Result<&Alloc, MemFault> {
        if p.is_static() {
            return self.statics.get((p.slot & !STATIC_BIT) as usize).ok_or(MemFault::Stale);
        }
        let a = self.slots.get(p.slot as usize).ok_or(MemFault::Stale)?;
        if a.generation != p.generation {
            return Err(MemFault::Stale);
        }
        if !a.live {
            return Err(MemFault::Dead(a.origin));
        }
        Ok(a)
    }

    pub fn alloc_of_mut(&mut self, p: Ptr) -> &mut Alloc {
        if p.is_static() {
            &mut self.statics[(p.slot & !STATIC_BIT) as usize]
        } else {
            &mut self.slots[p.slot as usize]
        }
    }

    /// Checks an access of `len` bytes at `p` and returns its absolute
    /// address in the pointer's space.
    pub fn check(&self, raw: u64, len: u64, write: bool) -> Result<(bool, u64, &Alloc), MemFault> {
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
        if a.flags & flags::EXTERNAL != 0
            && let Origin::External(d) = a.origin
        {
            return Err(MemFault::External(d, write));
        }
        Ok((p.is_static(), u64::from(a.base) + u64::from(p.off), a))
    }

    pub fn space(&self, stat: bool) -> &Space {
        if stat { &self.stat } else { &self.frame }
    }

    pub fn space_mut(&mut self, stat: bool) -> &mut Space {
        if stat { &mut self.stat } else { &mut self.frame }
    }

    /// Copies `len` bytes with their shadow (initialization and pointer
    /// marks) from `src` to `dst` (absolute addresses in their spaces).
    pub fn copy(&mut self, dst_st: bool, dst: u64, src_st: bool, src: u64, len: u64) {
        if dst_st == src_st {
            let sp = self.space_mut(dst_st);
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
        let (d, s) =
            if dst_st { (&mut self.stat, &self.frame) } else { (&mut self.frame, &self.stat) };
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
    pub fn zero(&mut self, st: bool, abs: u64, len: u64) {
        let sp = self.space_mut(st);
        sp.bytes[abs as usize..(abs + len) as usize].fill(0);
        sp.set_init(abs, len, true);
        sp.set_ptr(abs, len, false);
    }
}
