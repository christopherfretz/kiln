//! Deduction-attempt counts (feature `op-stats`, a measurement aid like
//! `kiln_oven`'s `trial_stats`; never built by default). Every function
//! template deduction (`deduce_spec`) is counted by its entry (a call, the
//! address of an overload set, a declaration, a conversion, explicit
//! arguments only), the step it ended at and its `Tdk`; the candidate sets
//! that resolved are counted with their template candidates, and a
//! template candidate that deduced but did not win is counted as a loser.
//! Repeats of the same (template, explicit arguments, argument types)
//! deduction within the translation unit are counted too. The table prints
//! to stderr as `deduce-stats:` lines when the compiling thread exits
//! (WP-657).

use std::cell::{Cell, RefCell};
use std::fmt::Write;
use std::hash::{Hash, Hasher};

use kiln_source::fx::{FxHashMap, FxHashSet, FxHasher};

use crate::templ::deduce::Tdk;

/// Nanoseconds as milliseconds, for printing.
#[allow(clippy::cast_precision_loss, reason = "a printed measurement")]
fn ms(ns: u64) -> f64 {
    ns as f64 / 1e6
}

#[derive(Default)]
struct Stats {
    /// (entry, step, tdk) → count, exclusive nanoseconds (nested
    /// deductions and template candidates not counted).
    rows: FxHashMap<(&'static str, &'static str, Tdk), (u64, u64)>,
    /// Deductions per template.
    per_template: FxHashMap<u32, u64>,
    /// Keys of the call deductions seen.
    seen: FxHashSet<u64>,
    repeats: u64,
    repeats_ok: u64,
    /// Candidate sets resolved, candidates in them, template candidates,
    /// template candidates that deduced and are viable, and those of them
    /// that did not win.
    sets: u64,
    cands: u64,
    tcands: u64,
    tviable: u64,
    tlosers: u64,
    /// Template candidates per resolved set: histogram by power of two.
    hist: [u64; 12],
    /// `pick_prev_function` by caller and the kind of the candidates'
    /// scope (WP-658).
    prev: FxHashMap<String, PrevRow>,
    /// Redeclaration-index queries, entries scanned for them, and the
    /// largest (keyed positions, heap bytes) seen (WP-658).
    index: (u64, u64, u64, u64),
    /// Named timers: name → (active depth, outermost nanoseconds, calls).
    timers: FxHashMap<&'static str, (u32, u64, u64)>,
    /// Free-form tallies: (kind, label) → count.
    tally: FxHashMap<(&'static str, String), u64>,
    /// Keyed call counts: kind → (calls, distinct keys) (WP-661).
    distinct: FxHashMap<&'static str, (u64, FxHashSet<u64>)>,
    /// Explicit-argument substitutions by key: (calls, a previous call
    /// succeeded creating no node) (WP-661).
    explicit: FxHashMap<u64, (u64, bool)>,
    /// Explicit-argument substitutions: calls, successes, successes that
    /// created no node, calls a memo of those would answer.
    explicit_n: (u64, u64, u64, u64),
    /// Explicit-argument substitutions the memo answered (WP-661).
    explicit_answered: u64,
    /// Redeclaration-index syncs: calls, calls with no journal event,
    /// slot events, retyped events, relinked events, events naming a
    /// watched declaration (WP-661).
    sync: (u64, u64, u64, u64, u64, u64),
    /// Retyped events naming a function-payload declaration; the largest
    /// retyped drain; the largest slot drain (WP-661 item 7).
    sync_more: (u64, u64, u64),
}

impl Drop for Stats {
    fn drop(&mut self) {
        let total: u64 =
            self.rows.iter().filter(|(k, _)| k.0 != "candidate").map(|(_, v)| v.0).sum();
        if total == 0 && self.sets == 0 {
            return;
        }
        let mut s = format!(
            "deduce-stats: deductions={total} distinct-templates={} repeats={} repeats-ok={} \
             sets={} candidates={} template-candidates={} template-viable={} template-losers={}\n",
            self.per_template.len(),
            self.repeats,
            self.repeats_ok,
            self.sets,
            self.cands,
            self.tcands,
            self.tviable,
            self.tlosers,
        );
        let mut v: Vec<_> = self.rows.iter().collect();
        v.sort_by(|a, b| {
            b.1.cmp(a.1).then_with(|| format!("{:?}", a.0).cmp(&format!("{:?}", b.0)))
        });
        for ((entry, step, tdk), (n, ns)) in v {
            let _ = writeln!(
                s,
                "deduce-stats:   {n:>10} {:>8.1}ms {entry:<10} {step:<14} {tdk:?}",
                ms(*ns)
            );
        }
        let _ = write!(s, "deduce-stats: template-candidates-per-set");
        for (i, n) in self.hist.iter().enumerate() {
            if *n != 0 {
                let lo = if i == 0 { 0 } else { 1u64 << (i - 1) };
                let _ = write!(s, " [{lo}..]={n}");
            }
        }
        s.push('\n');
        let mut t: Vec<_> = self.per_template.values().copied().collect();
        t.sort_unstable_by(|a, b| b.cmp(a));
        let _ = writeln!(s, "deduce-stats: per-template top: {:?}", &t[..t.len().min(12)]);
        let mut p: Vec<_> = self.prev.iter().collect();
        p.sort_by_key(|e| std::cmp::Reverse(e.1.scanned));
        for (at, r) in p {
            let _ = writeln!(
                s,
                "deduce-stats: pick-prev {at} calls={} scanned={} compared={} corresponded={} \
                 hits={} hit-pos-from-end={} max-scanned={} calls-over-32={} scanned-over-32={}",
                r.calls,
                r.scanned,
                r.compared,
                r.corresponded,
                r.hits,
                r.hit_pos,
                r.max,
                r.long_calls,
                r.long_scanned,
            );
        }
        if self.index.0 != 0 {
            let (q, n, e, b) = self.index;
            let _ = writeln!(
                s,
                "deduce-stats: redecl-index queries={q} returned={n} entries={e} bytes={b} \
                 bytes-per-entry={}",
                b / e.max(1)
            );
        }
        if self.sync.0 != 0 {
            let (n, e, a, b, c, w) = self.sync;
            let _ = writeln!(
                s,
                "deduce-stats: redecl-sync calls={n} empty={e} slot-events={a} retyped={b} \
                 relinked={c} watched={w} retyped-fn={} max-retyped={} max-slots={}",
                self.sync_more.0, self.sync_more.1, self.sync_more.2
            );
        }
        if self.explicit_n.0 != 0 {
            let (n, ok, zero, hits) = self.explicit_n;
            let _ = writeln!(
                s,
                "deduce-stats: explicit-subst calls={n} keys={} ok={ok} ok-no-node={zero} \
                 memo-hits={hits} answered={}",
                self.explicit.len(),
                self.explicit_answered
            );
        }
        let mut v: Vec<_> = self.distinct.iter().collect();
        v.sort_by_key(|e| e.0);
        for (kind, (n, keys)) in v {
            let _ = writeln!(s, "deduce-stats: distinct {kind} calls={n} keys={}", keys.len());
        }
        let mut v: Vec<_> = self.timers.iter().collect();
        v.sort_by_key(|e| std::cmp::Reverse(e.1.1));
        for (name, (_, ns, n)) in v {
            let _ = writeln!(s, "deduce-stats: timer {name:<28} {:>9.1}ms calls={n}", ms(*ns));
        }
        let mut v: Vec<_> = self.tally.iter().collect();
        v.sort_by(|a, b| b.1.cmp(a.1).then_with(|| a.0.cmp(b.0)));
        let mut shown: FxHashMap<&str, usize> = FxHashMap::default();
        for ((kind, label), n) in v {
            let c = shown.entry(kind).or_default();
            *c += 1;
            if *c <= 15 {
                let _ = writeln!(s, "deduce-stats: tally {kind} {n:>8} {label}");
            }
        }
        eprint!("{s}");
    }
}

/// One `pick_prev_function` row (WP-658).
#[derive(Default)]
struct PrevRow {
    calls: u64,
    /// Candidates passed in.
    scanned: u64,
    /// `fn_corresponds` calls, and those that returned true.
    compared: u64,
    corresponded: u64,
    /// Calls that returned a corresponding declaration, and the summed
    /// positions of those hits counted from the end of the candidates.
    hits: u64,
    hit_pos: u64,
    max: u64,
    /// Calls over at least 32 candidates, and their candidates.
    long_calls: u64,
    long_scanned: u64,
}

thread_local! {
    static COMPARED: Cell<(u64, u64)> = const { Cell::new((0, 0)) };
    static HIT: Cell<Option<u64>> = const { Cell::new(None) };
}

/// One `fn_corresponds` comparison, which returned `ok`.
pub(crate) fn compared(ok: bool) {
    COMPARED.with(|c| {
        let (n, t) = c.get();
        c.set((n + 1, t + u64::from(ok)));
    });
}

/// `pick_prev_function` returned the corresponding candidate `pos` places
/// from the end of its candidates (`None`: no corresponding one).
pub(crate) fn prev_hit(pos: Option<usize>) {
    HIT.with(|h| h.set(pos.map(|p| p as u64)));
}

/// A `pick_prev_function` from `at` over `n` candidates; returns the
/// comparison counts before it, for [`prev_end`].
/// A redeclaration-index query that returned `n` entries, with the index
/// at `entries` keyed positions and `bytes` heap bytes (WP-658).
pub(crate) fn index_query(n: usize, entries: usize, bytes: u64) {
    STATS.with(|s| {
        let i = &mut s.borrow_mut().index;
        i.0 += 1;
        i.1 += n as u64;
        if entries as u64 > i.2 {
            i.2 = entries as u64;
            i.3 = bytes;
        }
    });
}

pub(crate) fn prev_begin() -> (u64, u64) {
    HIT.with(|h| h.set(None));
    COMPARED.with(Cell::get)
}

/// Ends the `pick_prev_function` [`prev_begin`] started; `scope` names the
/// kind of the candidates' context.
pub(crate) fn prev_end(at: &std::panic::Location<'_>, scope: &str, n: usize, before: (u64, u64)) {
    let (c, t) = COMPARED.with(Cell::get);
    let hit = HIT.with(Cell::take);
    STATS.with(|s| {
        let mut s = s.borrow_mut();
        let e = s.prev.entry(format!("{}:{} {scope}", at.file(), at.line())).or_default();
        let n = n as u64;
        e.calls += 1;
        e.scanned += n;
        e.compared += c - before.0;
        e.corresponded += t - before.1;
        if let Some(p) = hit {
            e.hits += 1;
            e.hit_pos += p;
        }
        e.max = e.max.max(n);
        if n >= 32 {
            e.long_calls += 1;
            e.long_scanned += n;
        }
    });
}

thread_local! {
    static STATS: RefCell<Stats> = RefCell::new(Stats::default());
    static STEP: Cell<&'static str> = const { Cell::new("start") };
    /// Nanoseconds the frames nested in the current one took.
    static CHILD: Cell<u64> = const { Cell::new(0) };
}

/// A timed frame: a deduction or a template candidate.
pub(crate) struct Frame {
    step: &'static str,
    t0: std::time::Instant,
    child: u64,
}

fn frame_end(f: &Frame) -> u64 {
    let el = u64::try_from(f.t0.elapsed().as_nanos()).unwrap_or(u64::MAX);
    let inner = CHILD.with(|c| c.replace(f.child.saturating_add(el)));
    el.saturating_sub(inner)
}

/// Starts timing a template candidate.
pub(crate) fn cand_begin() -> Frame {
    Frame {
        step: STEP.with(Cell::get),
        t0: std::time::Instant::now(),
        child: CHILD.with(|c| c.replace(0)),
    }
}

/// Ends a template candidate whose deduction ended with `tdk`.
pub(crate) fn cand_end(f: &Frame, tdk: Tdk) {
    let ns = frame_end(f);
    STATS.with(|s| {
        let mut s = s.borrow_mut();
        let e = s.rows.entry(("candidate", "outside", tdk)).or_default();
        e.0 += 1;
        e.1 += ns;
    });
}

/// The step the current deduction is at.
pub(crate) fn step(s: &'static str) {
    STEP.with(|c| c.set(s));
}

/// Starts a deduction of template `ft`; returns the enclosing
/// deduction's step, for [`end`] to restore.
pub(crate) fn begin(ft: u32) -> Frame {
    STATS.with(|s| *s.borrow_mut().per_template.entry(ft).or_default() += 1);
    Frame {
        step: STEP.with(|c| c.replace("start")),
        t0: std::time::Instant::now(),
        child: CHILD.with(|c| c.replace(0)),
    }
}

/// Ends a deduction from `entry` with `tdk`; `key` the call deduction's
/// (template, explicit arguments, argument types) hash.
pub(crate) fn end(entry: &'static str, tdk: Tdk, key: Option<u64>, outer: &Frame) {
    let st = STEP.with(|c| c.replace(outer.step));
    let ns = frame_end(outer);
    STATS.with(|s| {
        let mut s = s.borrow_mut();
        let e =
            s.rows.entry((entry, if tdk == Tdk::Success { "ok" } else { st }, tdk)).or_default();
        e.0 += 1;
        e.1 += ns;
        if let Some(k) = key
            && !s.seen.insert(k)
        {
            s.repeats += 1;
            if tdk == Tdk::Success {
                s.repeats_ok += 1;
            }
        }
    });
}

/// The key of a call deduction.
pub(crate) fn key(ft: u32, explicit: Option<u32>, args: &[(kiln_ast::QualType, u8, bool)]) -> u64 {
    let mut h = FxHasher::default();
    ft.hash(&mut h);
    explicit.hash(&mut h);
    args.hash(&mut h);
    h.finish()
}

/// A resolved candidate set: `cands` candidates, `tcands` of them
/// templates, `tviable` of those viable, `tlosers` of those not chosen.
pub(crate) fn set(cands: u64, tcands: u64, tviable: u64, tlosers: u64) {
    STATS.with(|s| {
        let mut s = s.borrow_mut();
        s.sets += 1;
        s.cands += cands;
        s.tcands += tcands;
        s.tviable += tviable;
        s.tlosers += tlosers;
        let b = if tcands == 0 { 0 } else { (64 - tcands.leading_zeros()) as usize };
        s.hist[b.min(11)] += 1;
    });
}

/// An inclusive timer over the outermost activation of `name`.
pub(crate) struct Timer(&'static str, Option<std::time::Instant>);

/// Starts timer `name`.
pub(crate) fn timer(name: &'static str) -> Timer {
    let outer = STATS.with(|s| {
        let mut s = s.borrow_mut();
        let e = s.timers.entry(name).or_default();
        e.0 += 1;
        e.2 += 1;
        e.0 == 1
    });
    Timer(name, outer.then(std::time::Instant::now))
}

impl Drop for Timer {
    fn drop(&mut self) {
        let ns = self.1.map_or(0, |t| u64::try_from(t.elapsed().as_nanos()).unwrap_or(u64::MAX));
        STATS.with(|s| {
            let mut s = s.borrow_mut();
            let e = s.timers.entry(self.0).or_default();
            e.0 -= 1;
            e.1 += ns;
        });
    }
}

/// Counts one `label` under `kind`.
pub(crate) fn tally(kind: &'static str, label: String) {
    STATS.with(|s| *s.borrow_mut().tally.entry((kind, label)).or_default() += 1);
}

/// One call deduction answered by the pre-filter with `tdk`.
pub(crate) fn prefiltered(tdk: Tdk) {
    STATS.with(|s| s.borrow_mut().rows.entry(("call", "prefilter", tdk)).or_default().0 += 1);
}

/// A call of kind `kind` keyed `key` (WP-661: how often a per-key cache
/// would answer).
pub(crate) fn distinct(kind: &'static str, key: u64) {
    STATS.with(|s| {
        let mut s = s.borrow_mut();
        let e = s.distinct.entry(kind).or_default();
        e.0 += 1;
        e.1.insert(key);
    });
}

/// An explicit-argument substitution keyed `key` that succeeded (`ok`)
/// creating `nodes` AST nodes (WP-661).
pub(crate) fn explicit_subst(key: u64, ok: bool, nodes: u64) {
    STATS.with(|s| {
        let mut s = s.borrow_mut();
        let e = s.explicit.entry(key).or_default();
        e.0 += 1;
        let hit = e.1;
        if ok && nodes == 0 {
            e.1 = true;
        }
        let n = &mut s.explicit_n;
        n.0 += 1;
        n.1 += u64::from(ok);
        n.2 += u64::from(ok && nodes == 0);
        n.3 += u64::from(hit);
    });
}

/// A redeclaration-index sync that drained `slots`, `retyped` and
/// `relinked` events, `watched` of which name a watched declaration
/// (WP-661).
pub(crate) fn index_sync(slots: usize, retyped: usize, relinked: usize, watched: usize) {
    STATS.with(|s| {
        let y = &mut s.borrow_mut().sync;
        y.0 += 1;
        y.1 += u64::from(slots + retyped + relinked == 0);
        y.2 += slots as u64;
        y.3 += retyped as u64;
        y.4 += relinked as u64;
        y.5 += watched as u64;
    });
}

/// Item-7 detail of a sync: function-payload retyped events and drain sizes.
pub(crate) fn index_sync_more(retyped_fn: usize, retyped: usize, slots: usize) {
    STATS.with(|s| {
        let y = &mut s.borrow_mut().sync_more;
        y.0 += retyped_fn as u64;
        y.1 = y.1.max(retyped as u64);
        y.2 = y.2.max(slots as u64);
    });
}

/// A hash of `t`, for keyed counts.
pub(crate) fn key_of<T: Hash>(t: &T) -> u64 {
    let mut h = FxHasher::default();
    t.hash(&mut h);
    h.finish()
}

/// An explicit-argument substitution, `answered` by the memo (WP-661).
pub(crate) fn explicit_answered(answered: bool) {
    STATS.with(|s| s.borrow_mut().explicit_answered += u64::from(answered));
}
