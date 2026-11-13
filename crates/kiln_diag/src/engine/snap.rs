//! The snapshot encoding of [`DiagEngine`] (spec/diag.md "Snapshots",
//! spec/snap.md "Configured roots") and of [`DiagId`].
//!
//! The engine is a configured root: it borrows the source manager and
//! holds the consumer's sink and command-line policy, so it is not
//! `Snap`. [`DiagEngine::snap_put`] writes the state of a quiescent engine
//! and [`DiagEngine::snap_restore`] applies it to a fresh engine the
//! consumer built from its own options. Both name every field (an
//! exhaustive destructure), so a new field is a compile error here until
//! it is classified.
//!
//! | field | class | how |
//! |---|---|---|
//! | `sm` | configuration | the consumer's source manager (restored first) |
//! | `policy` | configuration | from the consumer's `-W` options (covered by the snapshot's configuration fingerprint) |
//! | `error_limit`, `clang_compat` | configuration | `-ferror-limit=`, `-fclang-compat=` |
//! | `state.sink` | configuration | where the consumer's diagnostics go |
//! | `state.seen` | state | the deduplication set: a diagnostic the prefix printed is not printed again |
//! | `pragmas` | state | `#pragma clang diagnostic` transition points, states and `push` stack, validated on restore |
//! | `has_pragmas` | rebuilt | from `pragmas` |
//! | `errors`, `uncompilable`, `unrecoverable`, `warnings` | transient | per-compilation output counts: not written; the consumer counts its own diagnostics (spec/diag.md "Snapshots") |
//! | `prefix_reported` | state | written as the producer's `warnings + errors` (its own value is 0 there): what the prefix reported, which a `-verify` consumer needs (it replays when not 0) |
//! | `fatal` | transient, at rest | must be false: a snapshot after a fatal error is refused |
//! | `holding`, `held`, `held_counts` | transient, at rest | must be 0, empty, 0: a snapshot inside a [`DiagEngine::hold`] is refused |

use std::collections::HashSet;
use std::sync::PoisonError;
use std::sync::atomic::Ordering;

use kiln_snap::{Desc, ErrorKind, Reader, Schema, SchemaHash, Snap, SnapError, Writer};
use kiln_source::Span;

use super::{DiagEngine, State};
use crate::ids::DiagId;
use crate::pragma::Pragmas;

/// A [`DiagId`] is its index in [`DiagId::ALL`], two bytes. The schema
/// descriptor carries the table's size and a hash of its names, so a
/// snapshot from a build with another table is refused.
impl Snap for DiagId {
    const SIZE: Option<usize> = Some(2);

    fn put(&self, w: &mut Writer) {
        w.u16(*self as u16);
    }

    fn get(r: &mut Reader<'_>) -> Result<Self, SnapError> {
        let i = r.u16()?;
        DiagId::ALL
            .get(usize::from(i))
            .copied()
            .ok_or_else(|| r.error::<Self>(ErrorKind::BadValue("diagnostic id out of range")))
    }

    const SCHEMA_HASH: u64 = {
        let mut h =
            SchemaHash::new("diag_id", "DiagId").note("u16 index").number(DiagId::ALL.len() as u64);
        let mut i = 0;
        while i < DiagId::ALL.len() {
            h = h.note(DiagId::ALL[i].name());
            i += 1;
        }
        h.finish()
    };

    fn schema(s: &mut Schema) {
        let mut h = 0xcbf2_9ce4_8422_2325u64;
        for id in DiagId::ALL {
            for &b in id.name().as_bytes().iter().chain(&[0xff]) {
                h ^= u64::from(b);
                h = h.wrapping_mul(0x0000_0100_0000_01b3);
            }
        }
        let note = format!("u16 index of {} ids, names {h:016x}", DiagId::ALL.len());
        s.register::<Self>(Desc::new("diag_id").hash(Self::SCHEMA_HASH).note(&note));
    }
}

/// Adds `field` to a decoding error's path.
fn at<T>(field: &'static str, r: Result<T, SnapError>) -> Result<T, SnapError> {
    r.map_err(|e| e.within(field))
}

impl DiagEngine<'_> {
    /// Whether the engine is at rest for a snapshot (spec/diag.md
    /// "Snapshots"): no [`hold`](Self::hold) open, nothing held, no fatal
    /// error. The error names the first field that is not at rest.
    ///
    /// # Errors
    /// The field that is not at rest, and why.
    pub fn snap_ready(&self) -> Result<(), String> {
        let holding = self.holding.load(Ordering::Acquire);
        if holding != 0 {
            return Err(format!("DiagEngine.holding: {holding} hold(s) open"));
        }
        let held = self.held_len();
        if held != 0 {
            return Err(format!("DiagEngine.held: {held} diagnostic(s) held"));
        }
        if let Some(i) = (0..4).find(|&i| self.held_count(i) != 0) {
            return Err(format!("DiagEngine.held_counts[{i}]: not 0"));
        }
        if self.has_fatal() {
            return Err("DiagEngine.fatal: a fatal error was emitted".to_owned());
        }
        Ok(())
    }

    /// Writes the engine's state (spec/snap.md "Configured roots"); the
    /// writer's mode decides the map order ([`Writer::canonical`]).
    ///
    /// # Errors
    /// [`Self::snap_ready`]'s: nothing is written then.
    pub fn snap_put(&self, w: &mut Writer) -> Result<(), String> {
        self.snap_ready()?;
        let DiagEngine {
            sm: _,
            policy: _,
            error_limit: _,
            clang_compat: _,
            state,
            pragmas,
            has_pragmas: _,
            errors: _,
            uncompilable: _,
            unrecoverable: _,
            warnings: _,
            fatal: _,
            holding: _,
            held: _,
            held_counts: _,
            prefix_reported,
        } = self;
        let st = state.lock().unwrap_or_else(PoisonError::into_inner);
        let State { sink: _, seen } = &*st;
        seen.put(w);
        drop(st);
        pragmas.read().unwrap_or_else(PoisonError::into_inner).put(w);
        // A restored engine's own prefix counts too (a snapshot of a
        // snapshot consumer).
        let reported =
            prefix_reported.saturating_add(self.warning_count()).saturating_add(self.error_count());
        w.put(&reported);
        Ok(())
    }

    /// Applies a snapshot written by [`Self::snap_put`] to this engine,
    /// which the consumer has just built from its own options (options
    /// diagnostics already emitted are kept, and so are its counts). It
    /// must be fresh: no pragma seen, nothing deduplicated yet, at rest.
    /// On an error the engine is unchanged.
    ///
    /// # Errors
    /// A corrupt snapshot ([`SnapError`], never a panic), or an engine
    /// that is not fresh (`ErrorKind::Invalid`).
    pub fn snap_restore(&mut self, r: &mut Reader<'_>) -> Result<(), SnapError> {
        let fresh = self.snap_ready().is_ok()
            && self.lock().seen.is_empty()
            && self.pragmas.read().unwrap_or_else(PoisonError::into_inner).is_empty();
        if !fresh {
            return Err(r.invalid::<Self>("restore into a used diagnostics engine"));
        }
        let seen: HashSet<(DiagId, Span, u64)> = at("DiagEngine.seen", r.get())?;
        let pragmas: Pragmas = at("DiagEngine.pragmas", r.get())?;
        if let Err(why) = pragmas.check() {
            return Err(r.invalid::<Self>(why).within("DiagEngine.pragmas"));
        }
        let reported: u32 = at("DiagEngine.prefix_reported", r.get())?;
        let DiagEngine {
            sm: _,
            policy: _,
            error_limit: _,
            clang_compat: _,
            state,
            pragmas: my_pragmas,
            has_pragmas,
            errors: _,
            uncompilable: _,
            unrecoverable: _,
            warnings: _,
            fatal: _,
            holding: _,
            held: _,
            held_counts: _,
            prefix_reported,
        } = self;
        *prefix_reported = reported;
        state.get_mut().unwrap_or_else(PoisonError::into_inner).seen = seen;
        has_pragmas.store(!pragmas.is_empty(), Ordering::Release);
        *my_pragmas.get_mut().unwrap_or_else(PoisonError::into_inner) = pragmas;
        Ok(())
    }

    /// Registers the snapshot's descriptor (the fields
    /// [`Self::snap_put`] writes) and those of the types it encodes.
    /// The constant schema hash of the root ([`Self::snap_schema`]).
    pub const SNAP_SCHEMA_HASH: u64 = SchemaHash::new("configured root", "DiagEngine")
        .field::<HashSet<(DiagId, Span, u64)>>("seen")
        .field::<Pragmas>("pragmas")
        .field::<u32>("prefix_reported")
        .finish();
    pub fn snap_schema(s: &mut Schema) {
        let d = Desc::new("configured root")
            .hash(Self::SNAP_SCHEMA_HASH)
            .field::<HashSet<(DiagId, Span, u64)>>("seen")
            .field::<Pragmas>("pragmas")
            .field::<u32>("prefix_reported");
        if s.register::<DiagEngine<'static>>(d) {
            HashSet::<(DiagId, Span, u64)>::schema(s);
            Pragmas::schema(s);
        }
    }
}
