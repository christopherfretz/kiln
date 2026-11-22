//! The evaluator's cost split (`-fconstexpr-profile-split`, WP-1864;
//! spec/oven.md "Cost split").
//!
//! A statistical profile of where the VM's time goes, by phase: the
//! instruction loop itself (decode, jump, register work), the
//! UB/lifetime/initialization checks, the memory model (pointer
//! provenance, allocation, typed copies), the memo table, the sema
//! bridge's callbacks, call frames, and compiling functions to KBC.
//!
//! **Sampling.** While the split is armed, the VM stores the current
//! [`Phase`] into one relaxed atomic byte at each instruction and at each
//! phase change inside it. A sampler thread wakes every [`INTERVAL`],
//! reads the byte and counts it. Nothing is timed on the evaluator's
//! thread: the cost when armed is one plain byte store per instruction
//! and phase change.
//!
//! **A build feature.** The stores exist only in a build with the
//! `split` feature (`cargo build --release -p kiln_driver --features
//! kiln_oven/split`). Without it the VM's loop is the code it was before
//! this module existed (no store, no branch, no clock), and
//! `-fconstexpr-profile-split` prints [`UNAVAILABLE`]. A loop chosen at
//! run time (a second instantiation, the first design) cost 3-4% of the
//! instructions of call-heavy evaluations unarmed, through changed
//! inlining (spec/oven.md "Cost split").
//!
//! The counts are a sample, not a measurement: a phase's share is its
//! sample count over all the VM's samples (`Phase::Idle` excluded), and
//! its time the share of the sampler's measured span spent outside
//! `Idle`. Shares below about 1% of a few thousand samples are noise.

use std::fmt::Write as _;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU64, Ordering};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// What the evaluator is doing ([`Split::set`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub(crate) enum Phase {
    /// Not in the VM (parsing, sema, codegen, or an evaluation's setup
    /// and freeze).
    Idle = 0,
    /// The instruction loop: decode, jump, register arithmetic and moves,
    /// `Step`, and the ops no other phase claims.
    Dispatch = 1,
    /// Checks that can only fail: initialization, pointer-ness, union
    /// active members, `const`/`volatile`, lifetime ends, enumeration
    /// ranges.
    Checks = 2,
    /// The memory model: a pointer's allocation and bounds (provenance),
    /// loads and stores of bytes, pointer arithmetic and comparison,
    /// allocation and release, typed copies, the memory builtins.
    Memory = 3,
    /// Memo-table lookups and inserts.
    Memo = 4,
    /// Sema-bridge callbacks: instantiating a body to compile, resolving
    /// a global, reflection meta operations.
    Bridge = 5,
    /// Call frames: entering a call (depth, registers, frame push) and
    /// returning from one.
    Frames = 6,
    /// Compiling a function to KBC (its body's instantiation is
    /// `Bridge`).
    Compile = 7,
}

/// Number of [`Phase`]s.
pub(crate) const PHASES: usize = 8;

/// The phases' names in the report, by discriminant.
const NAMES: [&str; PHASES] =
    ["idle", "dispatch", "checks", "memory", "memo", "bridge", "frames", "compile"];

/// The sampler's period.
pub(crate) const INTERVAL: Duration = Duration::from_micros(100);

/// The report line of a build without the `split` feature.
pub(crate) const UNAVAILABLE: &str =
    "constexpr: split unavailable (kiln was built without the kiln_oven/split feature)\n";

/// What the evaluator's thread and the sampler share.
#[derive(Debug)]
pub(crate) struct Shared {
    phase: AtomicU8,
    stop: AtomicBool,
    counts: [AtomicU64; PHASES],
}

impl Shared {
    /// Records the evaluator's phase (one relaxed byte store). Always
    /// inlined: it runs once per instruction of the armed loop.
    #[allow(clippy::inline_always)]
    #[inline(always)]
    pub(crate) fn set(&self, p: Phase) {
        self.phase.store(p as u8, Ordering::Relaxed);
    }

    /// The phase last recorded.
    #[inline]
    pub(crate) fn get(&self) -> Phase {
        match self.phase.load(Ordering::Relaxed) {
            1 => Phase::Dispatch,
            2 => Phase::Checks,
            3 => Phase::Memory,
            4 => Phase::Memo,
            5 => Phase::Bridge,
            6 => Phase::Frames,
            7 => Phase::Compile,
            _ => Phase::Idle,
        }
    }
}

/// The armed split: the shared state and its sampler thread (stopped and
/// joined on drop).
#[derive(Debug)]
pub(crate) struct Split {
    pub(crate) shared: Arc<Shared>,
    thread: Option<JoinHandle<Duration>>,
    started: Instant,
}

impl Split {
    /// Arms the split: starts the sampler. Without a thread (one that
    /// cannot be created) the split records no sample and says so.
    pub(crate) fn start() -> Split {
        let shared = Arc::new(Shared {
            phase: AtomicU8::new(Phase::Idle as u8),
            stop: AtomicBool::new(false),
            counts: Default::default(),
        });
        let s = Arc::clone(&shared);
        let thread = std::thread::Builder::new()
            .name("kiln-oven-split".into())
            .stack_size(64 * 1024)
            .spawn(move || {
                let t0 = Instant::now();
                while !s.stop.load(Ordering::Relaxed) {
                    std::thread::sleep(INTERVAL);
                    let p = s.phase.load(Ordering::Relaxed) as usize;
                    s.counts[p.min(PHASES - 1)].fetch_add(1, Ordering::Relaxed);
                }
                t0.elapsed()
            })
            .ok();
        Split { shared, thread, started: Instant::now() }
    }

    /// The samples counted so far, by phase.
    pub(crate) fn counts(&self) -> [u64; PHASES] {
        std::array::from_fn(|i| self.shared.counts[i].load(Ordering::Relaxed))
    }

    /// The report line (spec/driver.md "Constexpr profile"): `constexpr:
    /// split samples=N vm-ms=T <phase>=P% ...`, each phase's share of the
    /// VM's samples to one decimal (`-` with none).
    pub(crate) fn summary(&self) -> String {
        let c = self.counts();
        let total: u64 = c.iter().sum();
        let vm: u64 = c[1..].iter().sum();
        let span = self.started.elapsed().as_secs_f64() * 1e3;
        let vm_ms = if total == 0 { 0.0 } else { span * vm as f64 / total as f64 };
        let mut s = String::new();
        let _ = write!(s, "constexpr: split samples={vm} vm-ms={vm_ms:.1}");
        for (i, name) in NAMES.iter().enumerate().skip(1) {
            if vm == 0 {
                let _ = write!(s, " {name}=-");
            } else {
                let _ = write!(s, " {name}={:.1}%", 100.0 * c[i] as f64 / vm as f64);
            }
        }
        if self.thread.is_none() {
            s.push_str(" (no sampler thread)");
        }
        s.push('\n');
        s
    }
}

impl Drop for Split {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phases_round_trip_and_name_in_order() {
        let s = Split::start();
        for p in [
            Phase::Dispatch,
            Phase::Checks,
            Phase::Memory,
            Phase::Memo,
            Phase::Bridge,
            Phase::Frames,
            Phase::Compile,
            Phase::Idle,
        ] {
            s.shared.set(p);
            assert_eq!(s.shared.get(), p);
        }
        let line = s.summary();
        assert!(line.starts_with("constexpr: split samples="), "{line}");
        let keys: Vec<&str> = line
            .split_whitespace()
            .skip(4)
            .filter_map(|kv| kv.split_once('=').map(|(k, _)| k))
            .collect();
        assert_eq!(keys, ["dispatch", "checks", "memory", "memo", "bridge", "frames", "compile"]);
    }

    #[test]
    fn the_sampler_counts_the_phase_it_sees() {
        let s = Split::start();
        s.shared.set(Phase::Memory);
        let t = Instant::now();
        while s.counts()[Phase::Memory as usize] < 3 && t.elapsed() < Duration::from_secs(10) {
            std::thread::sleep(Duration::from_millis(1));
        }
        assert!(s.counts()[Phase::Memory as usize] >= 3);
        assert_eq!(s.counts()[Phase::Checks as usize], 0);
    }
}
