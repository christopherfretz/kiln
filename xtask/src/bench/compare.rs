//! Regression policy and the comparison and ratio tables (`spec/bench.md`
//! "Regression policy" and "Output").

use std::fmt::Write as _;

use super::record::{Baseline, InstCounters, Measurement};
use crate::util::header::Counter;

/// Regression thresholds in percent.
#[allow(clippy::struct_field_names)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Thresholds {
    /// Wall clock.
    pub time_pct: f64,
    /// Peak RSS.
    pub mem_pct: f64,
    /// The constexpr report's total `steps` (deterministic, so exact by default).
    pub steps_pct: f64,
    /// The constexpr report's total `bytes` and `peak_bytes`.
    pub bytes_pct: f64,
}

impl Default for Thresholds {
    fn default() -> Self {
        Self { time_pct: 5.0, mem_pct: 2.0, steps_pct: 0.0, bytes_pct: 2.0 }
    }
}

impl Thresholds {
    /// `self` with every threshold that `o` sets replaced.
    pub fn with(self, o: &ThresholdOverrides) -> Self {
        Self {
            time_pct: o.time_pct.unwrap_or(self.time_pct),
            mem_pct: o.mem_pct.unwrap_or(self.mem_pct),
            steps_pct: o.steps_pct.unwrap_or(self.steps_pct),
            bytes_pct: o.bytes_pct.unwrap_or(self.bytes_pct),
        }
    }
}

/// Thresholds set by an entry's `threshold-*:` directives or by the
/// `--threshold-*` options; unset ones fall through to the next level
/// (option, then entry directive, then [`Thresholds::default`]).
#[allow(clippy::struct_field_names)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ThresholdOverrides {
    pub time_pct: Option<f64>,
    pub mem_pct: Option<f64>,
    pub steps_pct: Option<f64>,
    pub bytes_pct: Option<f64>,
}

/// A threshold value: a finite, non-negative percentage.
pub fn parse_pct(v: &str) -> Option<f64> {
    v.parse().ok().filter(|p: &f64| p.is_finite() && *p >= 0.0)
}

/// Time changes smaller than this are never judged: process start-up jitter
/// alone reaches it on a loaded host, whatever the percentage.
pub const TIME_FLOOR_MS: f64 = 2.0;

/// How the current measurement of an entry relates to its baseline.
#[derive(Clone, Debug, PartialEq)]
pub enum Verdict {
    /// No baseline for this entry and compiler kind.
    New,
    /// Within thresholds.
    Ok,
    /// Better than the baseline beyond a threshold; consider `--record`.
    Improved,
    /// Worse beyond a threshold. Fails the run (unless `--record`).
    Regressed { time: bool, mem: bool, steps: bool, bytes: bool, inst: bool },
    /// Not comparable: the baseline was taken under different conditions
    /// (the reason says which). Reported, never fails.
    Advisory(String),
}

impl Verdict {
    pub fn label(&self) -> String {
        match self {
            Self::New => "new".into(),
            Self::Ok => "ok".into(),
            Self::Improved => "improved".into(),
            Self::Regressed { time, mem, steps, bytes, inst } => {
                let what: Vec<&str> = [
                    (time, "time"),
                    (mem, "mem"),
                    (steps, "steps"),
                    (bytes, "bytes"),
                    (inst, "inst"),
                ]
                .into_iter()
                .filter_map(|(on, name)| on.then_some(name))
                .collect();
                format!("REGRESSED ({})", what.join(", "))
            }
            Self::Advisory(why) => format!("advisory: {why}"),
        }
    }
}

/// Percentage change from `base` to `cur`.
pub fn delta_pct(base: f64, cur: f64) -> Option<f64> {
    (base > 0.0).then(|| (cur - base) / base * 100.0)
}

/// Applies the regression policy:
///
/// - Only comparable measurements are judged: same mode, flags, source,
///   compiler version and host. Otherwise the verdict is `Advisory`.
/// - Time regresses when even the **fastest** current run is slower than the
///   baseline's **median** by more than `time_pct` (and by at least
///   [`TIME_FLOOR_MS`]). Host noise only ever adds time, so a load spike moves
///   the median but rarely every run, while a real slowdown moves them all.
/// - Time improves when the current median beats the baseline's fastest run
///   by more than `time_pct` (and [`TIME_FLOOR_MS`]).
/// - Memory regresses (improves) when the peak RSS grows (shrinks) by more
///   than `mem_pct`. It is judged only when both sides have an RSS figure.
/// - The constexpr counters (kiln's report total) regress (improve) when
///   `steps` grows (shrinks) by more than `steps_pct`, or `bytes` or
///   `peak_bytes` by more than `bytes_pct`. They are judged only when both
///   sides have them. They are deterministic, so no noise allowance applies.
/// - The instantiation counters (`inst.*`, spec/sema-templates.md §11.4)
///   regress (improve) when `inst.count` grows (shrinks) at all — it is
///   exact, whatever the thresholds — or `inst.bytes` by more than
///   `bytes_pct`. They are judged only when both sides have them.
/// - Improvements never fail; they suggest re-recording.
pub fn judge(base: Option<&Baseline>, cur: &Measurement, t: Thresholds) -> Verdict {
    let Some(base) = base else { return Verdict::New };
    let b = &base.measurement;
    let why = if b.mode != cur.mode {
        Some(format!("mode changed ({} -> {})", b.mode.as_str(), cur.mode.as_str()))
    } else if b.flags != cur.flags {
        Some("flags changed since the baseline".to_owned())
    } else if b.source_hash != cur.source_hash {
        Some("entry source changed since the baseline".to_owned())
    } else if b.compiler_version != cur.compiler_version {
        Some(format!("compiler changed (`{}` -> `{}`)", b.compiler_version, cur.compiler_version))
    } else if b.host != cur.host {
        Some(format!(
            "host changed ({}/{} cpus -> {}/{} cpus)",
            b.host.arch, b.host.cpus, cur.host.arch, cur.host.cpus
        ))
    } else {
        None
    };
    if let Some(why) = why {
        return Verdict::Advisory(why);
    }
    let up = 1.0 + t.time_pct / 100.0;
    let down = 1.0 - t.time_pct / 100.0;
    let time_worse = cur.wall_ms_min > b.wall_ms_median * up
        && cur.wall_ms_min - b.wall_ms_median >= TIME_FLOOR_MS;
    let time_better = cur.wall_ms_median < b.wall_ms_min * down
        && b.wall_ms_min - cur.wall_ms_median >= TIME_FLOOR_MS;
    #[allow(clippy::cast_precision_loss)]
    let (mem_worse, mem_better) = match (b.max_rss_kb, cur.max_rss_kb) {
        (Some(bm), Some(cm)) => {
            let (bm, cm) = (bm as f64, cm as f64);
            (cm > bm * (1.0 + t.mem_pct / 100.0), cm < bm * (1.0 - t.mem_pct / 100.0))
        }
        _ => (false, false),
    };
    let (steps_worse, steps_better, bytes_worse, bytes_better) = match (&b.oven, &cur.oven) {
        (Some(bo), Some(co)) => {
            let (sw, sb) = beyond(bo.steps, co.steps, t.steps_pct);
            // The byte counters, minus those an older baseline lacks.
            let (mut bw, mut bb) = (false, false);
            for c in [Counter::Bytes, Counter::PeakBytes, Counter::PeakArena, Counter::FrozenBytes]
            {
                if !b.oven_missing.contains(&c) {
                    let (w, better) = beyond(bo.get(c), co.get(c), t.bytes_pct);
                    (bw, bb) = (bw || w, bb || better);
                }
            }
            (sw, sb, bw, bb)
        }
        _ => (false, false, false, false),
    };
    let (inst_worse, inst_better) = match (b.inst, cur.inst) {
        (Some(bi), Some(ci)) => {
            let (bw, bb) = beyond(bi.bytes, ci.bytes, t.bytes_pct);
            (ci.count > bi.count || bw, ci.count < bi.count || bb)
        }
        _ => (false, false),
    };
    if time_worse || mem_worse || steps_worse || bytes_worse || inst_worse {
        Verdict::Regressed {
            time: time_worse,
            mem: mem_worse,
            steps: steps_worse,
            bytes: bytes_worse,
            inst: inst_worse,
        }
    } else if time_better || mem_better || steps_better || bytes_better || inst_better {
        Verdict::Improved
    } else {
        Verdict::Ok
    }
}

/// Whether the counter `cur` is worse (above) or better (below) `base` by
/// more than `pct` percent.
#[allow(clippy::cast_precision_loss)]
fn beyond(base: u64, cur: u64, pct: f64) -> (bool, bool) {
    let (b, c) = (base as f64, cur as f64);
    (c > b * (1.0 + pct / 100.0), c < b * (1.0 - pct / 100.0))
}

/// A plain-text table: the first column left-aligned, the rest right-aligned.
pub fn render_table(header: &[&str], rows: &[Vec<String>]) -> String {
    let n = header.len();
    let mut width: Vec<usize> = header.iter().map(|h| h.chars().count()).collect();
    for r in rows {
        for (i, c) in r.iter().enumerate().take(n) {
            width[i] = width[i].max(c.chars().count());
        }
    }
    let mut out = String::new();
    let mut line = |cells: &[String]| {
        let mut l = String::new();
        for (i, c) in cells.iter().enumerate().take(n) {
            if i == 0 {
                let _ = write!(l, "{c:<w$}", w = width[0]);
            } else if i == n - 1 && !c.is_empty() && c.chars().any(char::is_alphabetic) {
                // A trailing text column (status) reads better left-aligned.
                let _ = write!(l, "  {c}");
            } else {
                let _ = write!(l, "  {c:>w$}", w = width[i]);
            }
        }
        out.push_str(l.trim_end());
        out.push('\n');
    };
    line(&header.iter().map(|h| (*h).to_owned()).collect::<Vec<_>>());
    for r in rows {
        line(r);
    }
    out
}

pub fn ms(x: f64) -> String {
    format!("{x:.1}")
}

#[allow(clippy::cast_precision_loss)]
pub fn mb(kb: Option<u64>) -> String {
    kb.map_or_else(|| "-".to_owned(), |k| format!("{:.1}", k as f64 / 1024.0))
}

pub fn pct(p: Option<f64>) -> String {
    p.map_or_else(|| "-".to_owned(), |p| format!("{p:+.1}%"))
}

#[allow(clippy::cast_precision_loss)]
pub fn ratio(test: f64, oracle: f64) -> Option<f64> {
    (oracle > 0.0).then(|| test / oracle)
}

/// Geometric mean of positive ratios.
#[allow(clippy::cast_precision_loss)]
pub fn geomean(xs: &[f64]) -> Option<f64> {
    let xs: Vec<f64> = xs.iter().copied().filter(|x| *x > 0.0 && x.is_finite()).collect();
    (!xs.is_empty()).then(|| (xs.iter().map(|x| x.ln()).sum::<f64>() / xs.len() as f64).exp())
}

/// One row of the compiler-vs-oracle comparison.
#[derive(Clone, Debug)]
pub struct RatioRow {
    pub id: String,
    pub mode: String,
    pub test: Option<Measurement>,
    pub oracle: Option<Measurement>,
}

impl RatioRow {
    pub fn time_ratio(&self) -> Option<f64> {
        ratio(self.test.as_ref()?.wall_ms_median, self.oracle.as_ref()?.wall_ms_median)
    }

    #[allow(clippy::cast_precision_loss)]
    pub fn mem_ratio(&self) -> Option<f64> {
        let t = self.test.as_ref()?.max_rss_kb?;
        let o = self.oracle.as_ref()?.max_rss_kb?;
        ratio(t as f64, o as f64)
    }

    /// The ratio-table cells: entry, mode, both medians, the time ratio,
    /// both peak RSS values, the memory ratio, and the compiler under test's
    /// instantiation totals ([`inst_cell`]).
    pub fn cells(&self) -> Vec<String> {
        let fmt_ratio = |r: Option<f64>| r.map_or_else(|| "-".to_owned(), |r| format!("{r:.2}x"));
        let t_ms = self.test.as_ref().map_or_else(|| "FAIL".to_owned(), |m| ms(m.wall_ms_median));
        let o_ms = self.oracle.as_ref().map_or_else(|| "FAIL".to_owned(), |m| ms(m.wall_ms_median));
        vec![
            self.id.clone(),
            self.mode.clone(),
            t_ms,
            o_ms,
            fmt_ratio(self.time_ratio()),
            self.test.as_ref().map_or_else(|| "-".to_owned(), |m| mb(m.max_rss_kb)),
            self.oracle.as_ref().map_or_else(|| "-".to_owned(), |m| mb(m.max_rss_kb)),
            fmt_ratio(self.mem_ratio()),
            inst_cell(self.test.as_ref().and_then(|m| m.inst)),
        ]
    }
}

/// `<count> / <KiB> KiB` of an instantiation report's totals, or `-`.
#[allow(clippy::cast_precision_loss)]
pub fn inst_cell(i: Option<InstCounters>) -> String {
    i.map_or_else(
        || "-".to_owned(),
        |i| format!("{} / {:.1} KiB", i.count, i.bytes as f64 / 1024.0),
    )
}

/// Column labels for the two compilers: their kinds, or `test`/`oracle`
/// when the kinds coincide.
pub fn ratio_labels(test_kind: &str, oracle_kind: &str) -> (String, String) {
    if test_kind == oracle_kind {
        ("test".into(), "oracle".into())
    } else {
        (test_kind.into(), oracle_kind.into())
    }
}

pub fn render_ratio_table(rows: &[RatioRow], t: &str, o: &str) -> String {
    let h = [
        "entry".to_owned(),
        "mode".to_owned(),
        format!("{t} ms"),
        format!("{o} ms"),
        format!("{t}/{o} time"),
        format!("{t} MB"),
        format!("{o} MB"),
        format!("{t}/{o} mem"),
        format!("{t} inst"),
    ];
    let hdr: Vec<&str> = h.iter().map(String::as_str).collect();
    let cells: Vec<Vec<String>> = rows.iter().map(RatioRow::cells).collect();
    let mut out = render_table(&hdr, &cells);
    let _ = writeln!(out, "{}", geomean_line(rows, t, o));
    out
}

fn geomean_line(rows: &[RatioRow], t: &str, o: &str) -> String {
    let time: Vec<f64> = rows.iter().filter_map(RatioRow::time_ratio).collect();
    let mem: Vec<f64> = rows.iter().filter_map(RatioRow::mem_ratio).collect();
    geomean_summary(&time, &mem, t, o)
}

/// `geometric mean <t>/<o>: time ..x over N entries, memory ..x over M entries`.
pub fn geomean_summary(time: &[f64], mem: &[f64], t: &str, o: &str) -> String {
    let g = |v: &[f64]| geomean(v).map_or_else(|| "n/a".to_owned(), |g| format!("{g:.2}x"));
    format!(
        "geometric mean {t}/{o}: time {} over {} entries, memory {} over {} entries",
        g(time),
        time.len(),
        g(mem),
        mem.len()
    )
}

#[cfg(test)]
mod tests {
    use super::super::record::Host;
    use super::super::record::tests::sample;
    use super::*;

    fn base(m: Measurement) -> Baseline {
        Baseline { measurement: m, recorded: "2026-10-28T000000".into() }
    }

    fn with(median: f64, min: f64, rss: Option<u64>) -> Measurement {
        let mut m = sample("clang++-19", median, rss);
        m.wall_ms_min = min;
        m
    }

    const T: Thresholds =
        Thresholds { time_pct: 5.0, mem_pct: 2.0, steps_pct: 0.0, bytes_pct: 2.0 };

    fn reg(time: bool, mem: bool) -> Verdict {
        Verdict::Regressed { time, mem, steps: false, bytes: false, inst: false }
    }

    #[test]
    fn time_regression_needs_every_run_slower() {
        let b = base(with(1000.0, 980.0, Some(100_000)));
        assert_eq!(judge(None, &with(1.0, 1.0, None), T), Verdict::New);
        assert_eq!(judge(Some(&b), &with(1040.0, 1000.0, Some(100_000)), T), Verdict::Ok);
        // Fastest run +6% over the baseline median: regressed.
        assert_eq!(judge(Some(&b), &with(1080.0, 1060.0, Some(100_000)), T), reg(true, false));
        // A noisy median alone (one fast run) is not a regression.
        assert_eq!(judge(Some(&b), &with(1300.0, 1040.0, Some(100_000)), T), Verdict::Ok);
        // Exactly at the threshold is not beyond it.
        assert_eq!(judge(Some(&b), &with(1100.0, 1050.0, Some(100_000)), T), Verdict::Ok);
        // Below the absolute floor nothing is judged, whatever the percentage.
        let tiny = base(with(10.0, 10.0, Some(100_000)));
        assert_eq!(judge(Some(&tiny), &with(11.9, 11.9, Some(100_000)), T), Verdict::Ok);
        assert_eq!(judge(Some(&tiny), &with(12.1, 12.1, Some(100_000)), T), reg(true, false));
        assert_eq!(judge(Some(&tiny), &with(7.0, 7.0, Some(100_000)), T), Verdict::Improved);
    }

    #[test]
    fn memory_threshold() {
        let b = base(with(1000.0, 980.0, Some(100_000)));
        assert_eq!(judge(Some(&b), &with(1000.0, 980.0, Some(102_001)), T), reg(false, true));
        assert_eq!(judge(Some(&b), &with(1000.0, 980.0, Some(102_000)), T), Verdict::Ok);
        assert_eq!(judge(Some(&b), &with(2000.0, 2000.0, Some(200_000)), T), reg(true, true));
        // No RSS on either side: memory is not judged.
        assert_eq!(judge(Some(&b), &with(1000.0, 980.0, None), T), Verdict::Ok);
        let loose = Thresholds { time_pct: 50.0, mem_pct: 150.0, ..T };
        assert_eq!(judge(Some(&b), &with(1400.0, 1400.0, Some(240_000)), loose), Verdict::Ok);
    }

    #[test]
    fn constexpr_counters() {
        use crate::util::oven_report::Counters;
        let counters = |steps, bytes, peak_bytes| Counters {
            steps,
            bytes,
            peak_bytes,
            calls: 7,
            allocs: 3,
            peak_arena: peak_bytes,
            frozen_bytes: 16,
            wall_us: 0,
            evaluations: 2,
        };
        let m = |c: Option<Counters>| {
            let mut m = with(1000.0, 980.0, Some(100_000));
            m.oven = c;
            m
        };
        let b = base(m(Some(counters(1000, 10_000, 5000))));
        assert_eq!(judge(Some(&b), &m(Some(counters(1000, 10_000, 5000))), T), Verdict::Ok);
        // Steps are exact by default: one more step regresses, one fewer improves.
        assert_eq!(
            judge(Some(&b), &m(Some(counters(1001, 10_000, 5000))), T),
            Verdict::Regressed { time: false, mem: false, steps: true, bytes: false, inst: false }
        );
        assert_eq!(judge(Some(&b), &m(Some(counters(999, 10_000, 5000))), T), Verdict::Improved);
        // Bytes and peak bytes share the 2% threshold.
        assert_eq!(judge(Some(&b), &m(Some(counters(1000, 10_200, 5100))), T), Verdict::Ok);
        let bytes =
            Verdict::Regressed { time: false, mem: false, steps: false, bytes: true, inst: false };
        assert_eq!(judge(Some(&b), &m(Some(counters(1000, 10_201, 5000))), T), bytes);
        assert_eq!(judge(Some(&b), &m(Some(counters(1000, 10_000, 5101))), T), bytes);
        assert_eq!(judge(Some(&b), &m(Some(counters(1000, 9_000, 5000))), T), Verdict::Improved);
        // So do the address-space peak and the frozen bytes ...
        let arena = Counters { peak_arena: 5101, ..counters(1000, 10_000, 5000) };
        assert_eq!(judge(Some(&b), &m(Some(arena)), T), bytes);
        let frozen = Counters { frozen_bytes: 17, ..counters(1000, 10_000, 5000) };
        assert_eq!(judge(Some(&b), &m(Some(frozen)), T), bytes);
        // ... unless the baseline predates them.
        let mut old = base(m(Some(counters(1000, 10_000, 5000))));
        old.measurement.oven_missing = vec![Counter::PeakArena, Counter::FrozenBytes];
        assert_eq!(judge(Some(&old), &m(Some(arena)), T), Verdict::Ok);
        assert_eq!(judge(Some(&old), &m(Some(frozen)), T), Verdict::Ok);
        // Loosened thresholds; one side without counters is not judged.
        let loose = Thresholds { steps_pct: 10.0, bytes_pct: 50.0, ..T };
        assert_eq!(judge(Some(&b), &m(Some(counters(1100, 15_000, 7500))), loose), Verdict::Ok);
        assert_eq!(judge(Some(&b), &m(None), T), Verdict::Ok);
        let v =
            Verdict::Regressed { time: true, mem: false, steps: true, bytes: true, inst: false };
        assert_eq!(v.label(), "REGRESSED (time, steps, bytes)");
    }

    /// `inst.count` is exact whatever the thresholds; `inst.bytes` shares
    /// `bytes_pct` (spec/sema-templates.md §11.4).
    #[test]
    fn instantiation_counters() {
        use super::super::record::InstCounters;
        let m = |i: Option<(u64, u64)>| {
            let mut m = with(1000.0, 980.0, Some(100_000));
            m.inst = i.map(|(count, bytes)| InstCounters { count, nodes: 5, bytes });
            m
        };
        let inst =
            Verdict::Regressed { time: false, mem: false, steps: false, bytes: false, inst: true };
        let b = base(m(Some((1025, 10_000))));
        assert_eq!(judge(Some(&b), &m(Some((1025, 10_000))), T), Verdict::Ok);
        assert_eq!(judge(Some(&b), &m(Some((1026, 10_000))), T), inst);
        assert_eq!(judge(Some(&b), &m(Some((1024, 10_000))), T), Verdict::Improved);
        assert_eq!(judge(Some(&b), &m(Some((1025, 10_200))), T), Verdict::Ok);
        assert_eq!(judge(Some(&b), &m(Some((1025, 10_201))), T), inst);
        assert_eq!(judge(Some(&b), &m(Some((1025, 9_000))), T), Verdict::Improved);
        // Loose thresholds widen the bytes, never the count.
        let loose = Thresholds { steps_pct: 50.0, bytes_pct: 50.0, ..T };
        assert_eq!(judge(Some(&b), &m(Some((1025, 14_000))), loose), Verdict::Ok);
        assert_eq!(judge(Some(&b), &m(Some((1026, 10_000))), loose), inst);
        // A baseline from before the metric, or a side without it: not judged.
        assert_eq!(judge(Some(&base(m(None))), &m(Some((1, 1))), T), Verdict::Ok);
        assert_eq!(judge(Some(&b), &m(None), T), Verdict::Ok);
        assert_eq!(inst.label(), "REGRESSED (inst)");
    }

    #[test]
    fn threshold_overrides() {
        let d = Thresholds::default();
        assert_eq!(d, Thresholds { time_pct: 5.0, mem_pct: 2.0, steps_pct: 0.0, bytes_pct: 2.0 });
        let entry =
            ThresholdOverrides { time_pct: Some(20.0), mem_pct: Some(4.0), ..Default::default() };
        let cli =
            ThresholdOverrides { time_pct: Some(50.0), steps_pct: Some(1.0), ..Default::default() };
        let t = d.with(&entry).with(&cli);
        assert_eq!(t, Thresholds { time_pct: 50.0, mem_pct: 4.0, steps_pct: 1.0, bytes_pct: 2.0 });
        assert_eq!(parse_pct("2.5"), Some(2.5));
        assert_eq!(parse_pct("0"), Some(0.0));
        for bad in ["-1", "x", "inf", "NaN", ""] {
            assert_eq!(parse_pct(bad), None, "{bad}");
        }
    }

    #[test]
    fn improvements_and_advisories() {
        let b = base(with(1000.0, 980.0, Some(100_000)));
        assert_eq!(judge(Some(&b), &with(900.0, 900.0, Some(100_000)), T), Verdict::Improved);
        // The median must beat the baseline's fastest run.
        assert_eq!(judge(Some(&b), &with(940.0, 900.0, Some(100_000)), T), Verdict::Ok);
        assert_eq!(judge(Some(&b), &with(1000.0, 980.0, Some(90_000)), T), Verdict::Improved);
        let mut c = with(5000.0, 5000.0, Some(100_000));
        c.compiler_version = "clang version 22".into();
        assert!(
            matches!(judge(Some(&b), &c, T), Verdict::Advisory(w) if w.contains("clang version 22"))
        );
        let mut c = with(5000.0, 5000.0, Some(100_000));
        c.host = Host { arch: "x86_64".into(), cpus: 8 };
        assert!(matches!(judge(Some(&b), &c, T), Verdict::Advisory(w) if w.contains("x86_64")));
        let mut c = with(5000.0, 5000.0, Some(100_000));
        c.source_hash = "fnv1a64:ffff".into();
        assert!(matches!(judge(Some(&b), &c, T), Verdict::Advisory(w) if w.contains("source")));
        let mut c = with(5000.0, 5000.0, Some(100_000));
        c.flags.push_str(" -O2");
        assert!(matches!(judge(Some(&b), &c, T), Verdict::Advisory(w) if w.contains("flags")));
    }

    #[test]
    fn tables_and_ratios() {
        let t = render_table(
            &["entry", "ms", "status"],
            &[
                vec!["oven/fib".into(), "712.3".into(), "ok".into()],
                vec!["pp/v".into(), "70.0".into(), "REGRESSED (time)".into()],
            ],
        );
        assert_eq!(
            t,
            "entry        ms  status\noven/fib  712.3  ok\npp/v       70.0  REGRESSED (time)\n"
        );
        assert_eq!(pct(delta_pct(100.0, 105.0)), "+5.0%");
        assert_eq!(pct(None), "-");
        assert_eq!(mb(Some(2048)), "2.0");
        let g = geomean(&[2.0, 8.0]).unwrap();
        assert!((g - 4.0).abs() < 1e-12);
        let row = RatioRow {
            id: "oven/fib".into(),
            mode: "syntax".into(),
            test: Some(with(200.0, 200.0, Some(50_000))),
            oracle: Some(with(100.0, 100.0, Some(100_000))),
        };
        assert_eq!(row.time_ratio(), Some(2.0));
        assert_eq!(row.mem_ratio(), Some(0.5));
        assert_eq!(row.cells()[8], "-");
        let mut inst = row.clone();
        inst.test.as_mut().unwrap().inst =
            Some(InstCounters { count: 1025, nodes: 1, bytes: 688_800 });
        assert_eq!(inst.cells()[8], "1025 / 672.7 KiB");
        let failed = RatioRow { test: None, ..row.clone() };
        let text = render_ratio_table(&[row, failed, inst], "kiln", "clang");
        assert!(text.contains("kiln inst") && text.contains("1025 / 672.7 KiB"), "{text}");
        assert!(
            text.contains("2.00x") && text.contains("0.50x") && text.contains("FAIL"),
            "{text}"
        );
        assert!(text.contains("time 2.00x over 2 entries"), "{text}");
        assert_eq!(ratio_labels("clang", "clang"), ("test".into(), "oracle".into()));
    }
}
