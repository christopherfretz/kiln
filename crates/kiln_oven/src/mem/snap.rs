//! The snapshot encoding of [`Memory`] (spec/oven.md "Snapshots",
//! spec/snap.md "Hand-written roots"), a part of the Oven's root
//! (`crate::snap`).
//!
//! Between evaluations only the static space holds objects: the frame
//! and heap spaces and their slot tables are emptied by
//! [`Memory::reset`] at the end of every outermost evaluation, so they
//! are transient and must be at rest ([`Memory::snap_ready`]).
//!
//! | field | class | how |
//! |---|---|---|
//! | `stat` | state | as data (bytes and the init, pointer and lifetime maps) |
//! | `statics`, `sfree`, `sholes`, `stat_peak` | state | as data |
//! | `unions`, `union_alts`, `union_epoch` | state | as data (static entries only at rest; the epoch stamps the union-path memo) |
//! | `vtags`, `vspill` | state | as data |
//! | `vtag_ix` | rebuilt | from `vtags` |
//! | `heap_ops`, `serial` | state | cumulative counters (the memo purity check and the frame serial allocator) |
//! | `tu_arena_peak` | state | the translation unit's high-water mark (`oven.heap-peak`) |
//! | `frame`, `heap` | transient, at rest | `top` 0; restored empty (no buffers) |
//! | `slots`, `free`, `live` | transient, at rest | only the dead slot 0, nothing free or live |
//! | `hslots`, `hfree`, `bins`, `heap_live`, `heap_count` | transient, at rest | empty, 0 |
//! | `unconstructed`, `trial` | transient, at rest | empty, no checkpoint open |
//! | `ctx`, `life_frame`, `life_hi`, `lax` | transient, at rest | 0, false |
//! | `heap_allocs`, `allocated`, `peak`, `arena_peak`, `refl`, `cur_frame` | transient | per evaluation, reset when one starts |
//! | `active_memo` | transient | a lookup hint, restored as none |
//! | `limit` | configuration | the consumer's `Budget::bytes` |

use std::cell::Cell;
use std::collections::VecDeque;

use kiln_snap::{Desc, Reader, Schema, SchemaHash, Snap, SnapError, Writer};
use kiln_source::fx::FxHashMap;

use super::{
    Alloc, Bins, DEAD0, GEN_MAX, GRAN, Memory, PTR_SPILL, PTR_TAGGED, SYNTH_SLOT, Space, Trial,
    UKey, UNBACKED,
};
use crate::ty::OTy;

/// Adds `field` to a decoding error's path.
fn at<T>(field: &'static str, r: Result<T, SnapError>) -> Result<T, SnapError> {
    r.map_err(|e| e.within(field))
}

impl Memory {
    /// The quiescence check of the memory (see the module docs): the
    /// first transient field that is not at rest.
    pub(crate) fn snap_ready(&self) -> Result<(), String> {
        let Memory {
            frame,
            heap,
            stat: _,
            slots,
            free,
            live,
            statics: _,
            sfree: _,
            sholes: _,
            stat_peak: _,
            hslots,
            hfree,
            bins,
            heap_live,
            heap_count,
            heap_allocs: _,
            heap_ops: _,
            ctx,
            unions: _,
            union_alts: _,
            union_epoch: _,
            active_memo: _,
            vtags: _,
            vtag_ix: _,
            vspill: _,
            allocated: _,
            peak: _,
            arena_peak: _,
            tu_arena_peak: _,
            refl: _,
            limit: _,
            cur_frame: _,
            serial: _,
            life_frame,
            life_hi,
            lax,
            unconstructed,
            trial,
        } = self;
        let busy = |what: &str| Err(format!("Memory.{what}"));
        if frame.top != 0 {
            return busy("frame: frame objects are live");
        }
        if heap.top != 0 || !hslots.is_empty() || *heap_live != 0 || *heap_count != 0 {
            return busy("heap: heap allocations are live");
        }
        if slots.len() != 1 || !free.is_empty() || !live.is_empty() {
            return busy("slots: frame slots are in use");
        }
        if !hfree.is_empty() || bins.lists.iter().any(|l| !l.is_empty()) || !bins.large.is_empty() {
            return busy("bins: heap blocks are pending reuse");
        }
        if *ctx != 0 || *life_frame || *life_hi != 0 || *lax {
            return busy("ctx: an evaluation context is open");
        }
        if !unconstructed.is_empty() {
            return busy("unconstructed: an evaluation's bases are recorded");
        }
        if !trial.cps.is_empty() || !trial.log.is_empty() {
            return busy("trial: a checkpoint is open");
        }
        Ok(())
    }

    /// Writes the state fields (see the module docs); the memory is at
    /// rest ([`Memory::snap_ready`]).
    pub(crate) fn snap_put(&self, w: &mut Writer) {
        let Memory {
            frame: _,
            heap: _,
            stat,
            slots: _,
            free: _,
            live: _,
            statics,
            sfree,
            sholes,
            stat_peak,
            hslots: _,
            hfree: _,
            bins: _,
            heap_live: _,
            heap_count: _,
            heap_allocs: _,
            heap_ops,
            ctx: _,
            unions,
            union_alts,
            union_epoch,
            active_memo: _,
            vtags,
            vtag_ix: _,
            vspill,
            allocated: _,
            peak: _,
            arena_peak: _,
            tu_arena_peak,
            refl: _,
            limit: _,
            cur_frame: _,
            serial,
            life_frame: _,
            life_hi: _,
            lax: _,
            unconstructed: _,
            trial: _,
        } = self;
        stat.put(w);
        statics.put(w);
        sfree.put(w);
        sholes.put(w);
        stat_peak.put(w);
        heap_ops.put(w);
        unions.put(w);
        union_alts.put(w);
        union_epoch.put(w);
        vtags.put(w);
        vspill.put(w);
        tu_arena_peak.put(w);
        serial.put(w);
    }

    /// Reads what [`Memory::snap_put`] wrote into a memory at rest with
    /// the memory limit `limit` (the consumer's), checked against the
    /// type table's length `ntypes`.
    pub(crate) fn snap_get(
        r: &mut Reader<'_>,
        limit: u64,
        ntypes: usize,
    ) -> Result<Memory, SnapError> {
        let stat: Space = at("Memory.stat", r.get())?;
        let statics: Vec<Alloc> = at("Memory.statics", r.get())?;
        let sfree: Vec<u32> = at("Memory.sfree", r.get())?;
        let sholes: Vec<(u64, u64)> = at("Memory.sholes", r.get())?;
        let stat_peak: u64 = at("Memory.stat_peak", r.get())?;
        let heap_ops: u64 = at("Memory.heap_ops", r.get())?;
        let unions: FxHashMap<UKey, u32> = at("Memory.unions", r.get())?;
        let union_alts: FxHashMap<UKey, u64> = at("Memory.union_alts", r.get())?;
        let union_epoch: u64 = at("Memory.union_epoch", r.get())?;
        let vtags: Vec<OTy> = at("Memory.vtags", r.get())?;
        let vspill: FxHashMap<u64, OTy> = at("Memory.vspill", r.get())?;
        let tu_arena_peak: u64 = at("Memory.tu_arena_peak", r.get())?;
        let serial: u32 = at("Memory.serial", r.get())?;
        if let Err(why) = check_static(&stat, &statics, &sfree, &sholes, ntypes) {
            return Err(r.invalid::<Memory>(why));
        }
        if vtags.len() > usize::from(PTR_SPILL - PTR_TAGGED)
            || vtags.iter().chain(vspill.values()).any(|t| t.0 as usize >= ntypes)
        {
            return Err(r.invalid::<Memory>("void pointer tags out of range"));
        }
        let ntags = vtags.len();
        if stat
            .ptr
            .iter()
            .any(|&m| m >= PTR_TAGGED && m != PTR_SPILL && usize::from(m - PTR_TAGGED) >= ntags)
        {
            return Err(r.invalid::<Memory>("a pointer mark names an unknown void pointer tag"));
        }
        let vtag_ix = vtags.iter().enumerate().map(|(i, &t)| (t, i as u8 + PTR_TAGGED)).collect();
        let fresh = Memory::new(limit);
        Ok(Memory {
            frame: Space::default(),
            heap: Space::default(),
            stat,
            slots: vec![DEAD0],
            free: VecDeque::new(),
            live: Vec::new(),
            statics,
            sfree,
            sholes,
            stat_peak,
            hslots: Vec::new(),
            hfree: VecDeque::new(),
            bins: Bins::default(),
            heap_live: 0,
            heap_count: 0,
            heap_allocs: 0,
            heap_ops,
            ctx: 0,
            unions,
            union_alts,
            union_epoch,
            active_memo: Cell::new(fresh.active_memo.get()),
            vtags,
            vtag_ix,
            vspill,
            allocated: 0,
            peak: 0,
            arena_peak: 0,
            tu_arena_peak,
            refl: 0,
            limit,
            cur_frame: 0,
            serial,
            life_frame: false,
            life_hi: 0,
            lax: false,
            unconstructed: Vec::new(),
            trial: Trial::default(),
        })
    }

    /// Registers the encoding of [`Memory::snap_put`].
    /// The constant schema hash of the root ([`Self::snap_schema`]).
    pub(crate) const SNAP_SCHEMA_HASH: u64 = SchemaHash::new("root", "Memory")
        .field::<Space>("stat")
        .field::<Vec<Alloc>>("statics")
        .field::<Vec<u32>>("sfree")
        .field::<Vec<(u64, u64)>>("sholes")
        .field::<u64>("stat_peak")
        .field::<u64>("heap_ops")
        .field::<FxHashMap<UKey, u32>>("unions")
        .field::<FxHashMap<UKey, u64>>("union_alts")
        .field::<u64>("union_epoch")
        .field::<Vec<OTy>>("vtags")
        .skip("vtag_ix", "FxHashMap<OTy, u8>")
        .field::<FxHashMap<u64, OTy>>("vspill")
        .field::<u64>("tu_arena_peak")
        .field::<u32>("serial")
        .finish();
    pub(crate) fn snap_schema(s: &mut Schema) {
        let d = Desc::new("root")
            .hash(Self::SNAP_SCHEMA_HASH)
            .field::<Space>("stat")
            .field::<Vec<Alloc>>("statics")
            .field::<Vec<u32>>("sfree")
            .field::<Vec<(u64, u64)>>("sholes")
            .field::<u64>("stat_peak")
            .field::<u64>("heap_ops")
            .field::<FxHashMap<UKey, u32>>("unions")
            .field::<FxHashMap<UKey, u64>>("union_alts")
            .field::<u64>("union_epoch")
            .field::<Vec<OTy>>("vtags")
            .skip::<FxHashMap<OTy, u8>>("vtag_ix")
            .field::<FxHashMap<u64, OTy>>("vspill")
            .field::<u64>("tu_arena_peak")
            .field::<u32>("serial");
        if s.register::<Memory>(d) {
            Space::schema(s);
            Vec::<Alloc>::schema(s);
            Vec::<u32>::schema(s);
            Vec::<(u64, u64)>::schema(s);
            u64::schema(s);
            FxHashMap::<UKey, u32>::schema(s);
            FxHashMap::<UKey, u64>::schema(s);
            Vec::<OTy>::schema(s);
            FxHashMap::<u64, OTy>::schema(s);
            u32::schema(s);
        }
    }
}

/// The invariants the accesses of the static space rely on without a
/// check: the maps cover the bytes granule by granule, every allocation
/// with bytes lies below the top, slot and generation numbers fit a
/// pointer's encoding, the free lists name dead slots and free ranges.
fn check_static(
    stat: &Space,
    statics: &[Alloc],
    sfree: &[u32],
    sholes: &[(u64, u64)],
    ntypes: usize,
) -> Result<(), &'static str> {
    let len = stat.bytes.len() as u64;
    let grans = (len / GRAN) as usize;
    if !len.is_multiple_of(GRAN)
        || stat.init.len() != grans
        || stat.ptr.len() != grans
        || !(stat.life.is_empty() || stat.life.len() == grans)
        || stat.top > len
    {
        return Err("static space maps do not cover its bytes");
    }
    // Slot indices above `SYNTH_SLOT` are reserved for pointers that
    // point at nothing.
    if statics.len() >= SYNTH_SLOT as usize {
        return Err("too many static slots");
    }
    for a in statics {
        let end = u64::from(a.base) + u64::from(a.size).max(1);
        if (a.base != UNBACKED && end > stat.top) || a.generation > GEN_MAX {
            return Err("static allocation outside the static space");
        }
        if a.ty.0 as usize >= ntypes {
            return Err("static allocation of an unknown type");
        }
    }
    if sfree.iter().any(|&i| statics.get(i as usize).is_none_or(|a| a.live)) {
        return Err("free static slot list names a live or unknown slot");
    }
    let mut prev_end = 0;
    for &(b, l) in sholes {
        if b < prev_end || l == 0 || b.checked_add(l).is_none_or(|e| e > stat.top) {
            return Err("static free ranges out of order or range");
        }
        prev_end = b + l;
    }
    Ok(())
}
