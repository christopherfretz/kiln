//! The per-field decode timer of the snapshot load (`KILN_SNAP_TIMING`,
//! spec/snap.md "Load timing"): instrumentation, no semantic state. A
//! hand-written root reads its fields through [`timed`] (and names its
//! rebuilds with [`timed_part`]); while [`set_field_timing`] is on for
//! the thread, each adds its time and encoded size to the rows
//! [`take_field_times`] returns. Off, each is one thread-local read.

use std::cell::{Cell, RefCell};
use std::time::{Duration, Instant};

use crate::{Reader, Snap, SnapError};

thread_local! {
    static FIELD_TIMING: Cell<bool> = const { Cell::new(false) };
    static FIELD_TIMES: RefCell<Vec<(&'static str, Duration, usize)>> =
        const { RefCell::new(Vec::new()) };
}

/// Turns the per-field decode timer on or off for this thread.
pub fn set_field_timing(on: bool) {
    FIELD_TIMING.with(|c| c.set(on));
}

/// Each timed field decoded on this thread since the last call while
/// [`set_field_timing`] was on: its name, decode time and encoded size.
#[must_use]
pub fn take_field_times() -> Vec<(&'static str, Duration, usize)> {
    FIELD_TIMES.with(|t| std::mem::take(&mut *t.borrow_mut()))
}

/// Reads the field `field` (adding it to a decoding error's path), and
/// records its decode time and size while the timer is on.
#[inline]
pub fn timed<T: Snap>(r: &mut Reader<'_>, field: &'static str) -> Result<T, SnapError> {
    if !FIELD_TIMING.with(Cell::get) {
        return r.get().map_err(|e| e.within(field));
    }
    let started = Instant::now();
    let from = r.offset();
    let v = r.get().map_err(|e| e.within(field));
    let row = (field, started.elapsed(), r.offset() - from);
    FIELD_TIMES.with(|t| t.borrow_mut().push(row));
    v
}

/// Runs `f`, a named part of a decode that reads no bytes (a table
/// rebuild, a check), recording its time while the timer is on.
#[inline]
pub fn timed_part<T>(part: &'static str, f: impl FnOnce() -> T) -> T {
    if !FIELD_TIMING.with(Cell::get) {
        return f();
    }
    let started = Instant::now();
    let v = f();
    let row = (part, started.elapsed(), 0);
    FIELD_TIMES.with(|t| t.borrow_mut().push(row));
    v
}
