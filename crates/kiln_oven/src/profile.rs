//! The constant-evaluation profile (spec/oven.md "Profile counters",
//! WP-1116): `-fconstexpr-stats` and `-fconstexpr-profile`.
//!
//! The sink lives in the WP-1115 hooks (`hooks.rs`): with
//! [`Level::Calls`] every call the VM enters and every top-level
//! evaluation opens a mark there, and its end charges a [`Row`] here —
//! of its function (`Oven::funcs` index) or of its evaluation site
//! (location and kind). Steps are exact (the VM's step counter read at
//! each end), times are wall-clock nanoseconds. The summary needs no
//! per-call hook: it reads the per-evaluation records ([`EvalStats`]),
//! the per-context totals ([`crate::stats::JobTotals`]), the memo
//! table's counters and the meta-operation and compile counts kept
//! here.
//!
//! [`EvalStats`]: crate::stats::EvalStats

use std::fmt::Write as _;

use kiln_ast::{Ast, DeclId};
use kiln_source::Loc;
use kiln_source::fx::FxHashMap;

use crate::{EvalKind, Oven, notes};

/// What the profile records (the driver's flags).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Level {
    /// Nothing (no flag).
    #[default]
    Off,
    /// The summary's counters (`-fconstexpr-stats`): no per-call hook.
    Stats,
    /// Per function and per evaluation site too (`-fconstexpr-profile`).
    Calls,
}

/// One profile row's counters: a function's or an evaluation site's.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Row {
    /// Calls entered (a function) or evaluations (a site); a memo hit
    /// counts as a call.
    pub count: u64,
    /// Calls answered from the memo table.
    pub memo_hits: u64,
    /// Evaluations that failed (a site).
    pub failed: u64,
    /// Steps from entry to return, its callees' included; a recursive
    /// call's are charged once, by the outermost activation.
    pub incl_steps: u64,
    /// Steps outside any callee (a site: outside any call).
    pub self_steps: u64,
    /// Nanoseconds from entry to return (as `incl_steps`).
    pub incl_ns: u64,
    /// Nanoseconds outside any callee and nested evaluation.
    pub self_ns: u64,
    /// The most abstract-memory bytes live at once above those live at
    /// entry (a site: the evaluation's `peak_bytes`).
    pub peak: u64,
    /// Activations not yet returned (recursion).
    active: u32,
}

impl Row {
    /// Charges the end of one activation.
    fn charge(&mut self, s: Spent) {
        self.active = self.active.saturating_sub(1);
        self.self_steps += s.steps.saturating_sub(s.child_steps);
        self.self_ns += s.ns.saturating_sub(s.child_ns);
        if self.active == 0 {
            self.incl_steps += s.steps;
            self.incl_ns += s.ns;
        }
        self.peak = self.peak.max(s.peak);
    }
}

/// What one activation spent ([`Row::charge`]).
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Spent {
    pub(crate) steps: u64,
    pub(crate) child_steps: u64,
    pub(crate) ns: u64,
    pub(crate) child_ns: u64,
    pub(crate) peak: u64,
}

/// The profile of a translation unit (owned by the hooks).
#[derive(Debug, Default)]
pub(crate) struct Profile {
    pub(crate) level: Level,
    /// By `Oven::funcs` index.
    funcs: Vec<Row>,
    /// By first evaluation; keyed in `site_ix`.
    sites: Vec<(Loc, EvalKind, Option<DeclId>, Row)>,
    site_ix: FxHashMap<(Loc, EvalKind), u32>,
    /// Reflection meta operations run.
    pub(crate) meta_ops: u64,
    /// Functions compiled to VM code, and the nanoseconds it took.
    pub(crate) compiles: u64,
    pub(crate) compile_ns: u64,
}

impl Profile {
    pub(crate) fn new(level: Level) -> Profile {
        Profile { level, ..Profile::default() }
    }

    /// A call of `func` begins (or was answered from the memo table).
    pub(crate) fn func_enter(&mut self, func: u32) -> &mut Row {
        let i = func as usize;
        if self.funcs.len() <= i {
            self.funcs.resize(i + 1, Row::default());
        }
        let r = &mut self.funcs[i];
        r.count += 1;
        r.active += 1;
        r
    }

    /// A call of `func` ends.
    pub(crate) fn func_end(&mut self, func: u32, s: Spent) {
        if let Some(r) = self.funcs.get_mut(func as usize) {
            r.charge(s);
        }
    }

    /// An evaluation of `kind` at `loc` (of `object`'s initializer) ends.
    pub(crate) fn site_end(
        &mut self,
        loc: Loc,
        kind: EvalKind,
        object: Option<DeclId>,
        ok: bool,
        s: Spent,
    ) {
        let n = self.sites.len() as u32;
        let i = *self.site_ix.entry((loc, kind)).or_insert(n) as usize;
        if i == self.sites.len() {
            self.sites.push((loc, kind, object, Row::default()));
        }
        let r = &mut self.sites[i].3;
        r.count += 1;
        r.failed += u64::from(!ok);
        r.active += 1;
        r.charge(s);
    }
}

/// One row of the rendered profile.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProfileRow {
    /// A function's qualified name; a site's kind (`static_assert`, ...),
    /// with the variable's qualified name for an initializer.
    pub name: String,
    /// Whether the function is an immediate function (a site: `false`).
    pub immediate: bool,
    /// The function's declaration or the evaluated construct.
    pub loc: Loc,
    /// The counters (functions sharing a declaration merged).
    pub row: Row,
}

/// The profile's rows, ranked: inclusive steps descending, then count
/// descending, then location (file, line, column), then name.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProfileRows {
    /// Functions.
    pub funcs: Vec<ProfileRow>,
    /// Top-level evaluation sites.
    pub sites: Vec<ProfileRow>,
    /// The TU's totals (the `total` line).
    pub totals: ProfileTotals,
}

/// The totals of a [`ProfileRows`]: recorded evaluations from
/// [`crate::stats::EvalTotals`] (each context's own steps and time, so a
/// nested evaluation is not counted twice), unrecorded trial evaluations
/// from [`crate::stats::JobTotals`]. Function rows include the trials'
/// calls; site rows do not.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProfileTotals {
    /// Recorded evaluations.
    pub evaluations: u64,
    /// Their steps.
    pub steps: u64,
    /// Their wall time in microseconds.
    pub wall_us: u64,
    /// Trial evaluations (not recorded: folds, constant-initialization
    /// checks).
    pub trials: u64,
    /// Their steps.
    pub trial_steps: u64,
}

/// How [`ProfileRows::render`] writes the report.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RenderOptions {
    /// NDJSON (else text).
    pub json: bool,
    /// Rows shown per table (0: all).
    pub limit: usize,
}

/// A location as the report spells it: (file, line, column).
pub type LocOf<'a> = &'a dyn Fn(Loc) -> Option<(String, u32, u32)>;

impl ProfileRows {
    /// Ranks the rows (the order of [`ProfileRows`]).
    fn rank(v: &mut Vec<ProfileRow>, loc: LocOf<'_>) {
        let mut keyed: Vec<_> = std::mem::take(v).into_iter().map(|r| (loc(r.loc), r)).collect();
        keyed.sort_by(|(la, x), (lb, y)| {
            y.row
                .incl_steps
                .cmp(&x.row.incl_steps)
                .then(y.row.count.cmp(&x.row.count))
                .then_with(|| la.cmp(lb))
                .then_with(|| x.name.cmp(&y.name))
        });
        *v = keyed.into_iter().map(|(_, r)| r).collect();
    }

    /// The report of main file `file` (its file name is the header's).
    pub fn render(&self, file: &str, opts: RenderOptions, loc: LocOf<'_>) -> String {
        let mut funcs = self.funcs.clone();
        let mut sites = self.sites.clone();
        Self::rank(&mut funcs, loc);
        Self::rank(&mut sites, loc);
        let take = |n: usize| if opts.limit == 0 { n } else { n.min(opts.limit) };
        let (nf, ns) = (take(funcs.len()), take(sites.len()));
        let spell = |l: Loc, base: bool| {
            loc(l).map_or_else(String::new, |(f, line, col)| {
                let f = if base { f.rsplit('/').next().unwrap_or(&f).to_owned() } else { f };
                format!("{f}:{line}:{col}")
            })
        };
        let t = self.totals;
        let calls: u64 = funcs.iter().map(|r| r.row.count).sum();
        let mut out = String::new();
        if opts.json {
            for r in &funcs[..nf] {
                let _ = writeln!(
                    out,
                    "{{\"function\":{},\"consteval\":{},\"loc\":{},\"calls\":{},\"memo_hits\":{},\"incl_steps\":{},\"self_steps\":{},\"incl_us\":{},\"self_us\":{},\"peak_bytes\":{}}}",
                    json_str(&r.name),
                    r.immediate,
                    json_str(&spell(r.loc, false)),
                    r.row.count,
                    r.row.memo_hits,
                    r.row.incl_steps,
                    r.row.self_steps,
                    r.row.incl_ns / 1000,
                    r.row.self_ns / 1000,
                    r.row.peak
                );
            }
            for r in &sites[..ns] {
                let _ = writeln!(
                    out,
                    "{{\"site\":{},\"loc\":{},\"evaluations\":{},\"failed\":{},\"incl_steps\":{},\"self_steps\":{},\"incl_us\":{},\"self_us\":{},\"peak_bytes\":{}}}",
                    json_str(&r.name),
                    json_str(&spell(r.loc, false)),
                    r.row.count,
                    r.row.failed,
                    r.row.incl_steps,
                    r.row.self_steps,
                    r.row.incl_ns / 1000,
                    r.row.self_ns / 1000,
                    r.row.peak
                );
            }
            let _ = writeln!(
                out,
                "{{\"total\":true,\"file\":{},\"functions\":{},\"sites\":{},\"evaluations\":{},\"steps\":{},\"wall_us\":{},\"trials\":{},\"trial_steps\":{},\"calls\":{}}}",
                json_str(file),
                funcs.len(),
                sites.len(),
                t.evaluations,
                t.steps,
                t.wall_us,
                t.trials,
                t.trial_steps,
                calls
            );
            return out;
        }
        let _ = writeln!(out, "===-- kiln constexpr profile: {file} --===");
        let head = |out: &mut String, what: &str, n: usize, all: usize, count: &str, aux: &str| {
            let _ = writeln!(out, "{what} (top {n} of {all}, by inclusive steps):");
            let _ = writeln!(
                out,
                "{:>8} {:>7} {:>12} {:>12} {:>10} {:>10} {:>11}  {}",
                count, aux, "incl steps", "self steps", "incl ms", "self ms", "peak bytes", what
            );
        };
        let more = |out: &mut String, n: usize, all: usize| {
            if n < all {
                let _ =
                    writeln!(out, "  ... {} more rows (use -fconstexpr-profile-limit=0)", all - n);
            }
        };
        let line = |out: &mut String, r: &ProfileRow, aux: u64, tag: &str| {
            let _ = writeln!(
                out,
                "{:>8} {:>7} {:>12} {:>12} {:>10.3} {:>10.3} {:>11}  {}{}   ({})",
                r.row.count,
                aux,
                r.row.incl_steps,
                r.row.self_steps,
                ms(r.row.incl_ns),
                ms(r.row.self_ns),
                r.row.peak,
                r.name,
                tag,
                spell(r.loc, true)
            );
        };
        head(&mut out, "functions", nf, funcs.len(), "calls", "memo");
        for r in &funcs[..nf] {
            line(&mut out, r, r.row.memo_hits, if r.immediate { " [consteval]" } else { "" });
        }
        more(&mut out, nf, funcs.len());
        head(&mut out, "evaluation sites", ns, sites.len(), "evals", "failed");
        for r in &sites[..ns] {
            line(&mut out, r, r.row.failed, "");
        }
        more(&mut out, ns, sites.len());
        let _ = writeln!(
            out,
            "  total: {} evaluations, {} steps, {:.3} ms; {} trials, {} steps; {calls} calls",
            t.evaluations,
            t.steps,
            ms(t.wall_us * 1000),
            t.trials,
            t.trial_steps
        );
        out
    }
}

impl Oven {
    /// Starts recording the profile at `level` (call before the first
    /// evaluation; [`Level::Off`] stops and drops it).
    pub fn set_profile(&mut self, level: Level) {
        self.hooks.prof = (level != Level::Off).then(|| Box::new(Profile::new(level)));
        self.hooks.refresh();
    }

    /// The level recorded at.
    pub fn profile_level(&self) -> Level {
        self.hooks.prof.as_ref().map_or(Level::Off, |p| p.level)
    }

    /// The profile's rows (`None` below [`Level::Calls`]): functions
    /// sharing a declaration are merged, functions without one (an
    /// expression's code) are `<expression>`.
    pub fn profile_rows(&self, ast: &Ast) -> Option<ProfileRows> {
        let p = self.hooks.prof.as_ref().filter(|p| p.level == Level::Calls)?;
        let mut by_decl: FxHashMap<Option<DeclId>, usize> = FxHashMap::default();
        let mut funcs: Vec<ProfileRow> = Vec::new();
        for (i, r) in p.funcs.iter().enumerate() {
            if r.count == 0 {
                continue;
            }
            let decl = self.funcs.get(i).and_then(|f| f.decl);
            let at = *by_decl.entry(decl).or_insert_with(|| {
                let (name, immediate, loc) = match decl {
                    Some(d) => (
                        notes::qualified_name(ast, d),
                        ast.decls.function(ast.latest(d)).is_some_and(|f| f.spec.is_immediate()),
                        ast.decls.get(d).loc(),
                    ),
                    None => ("<expression>".to_owned(), false, Loc::INVALID),
                };
                funcs.push(ProfileRow { name, immediate, loc, row: Row::default() });
                funcs.len() - 1
            });
            let m = &mut funcs[at].row;
            m.count += r.count;
            m.memo_hits += r.memo_hits;
            m.incl_steps += r.incl_steps;
            m.self_steps += r.self_steps;
            m.incl_ns += r.incl_ns;
            m.self_ns += r.self_ns;
            m.peak = m.peak.max(r.peak);
        }
        let sites = p
            .sites
            .iter()
            .map(|&(loc, kind, object, row)| {
                let name = match object.filter(|_| kind == EvalKind::Init) {
                    Some(d) => format!("{} {}", kind.as_str(), notes::qualified_name(ast, d)),
                    None => kind.as_str().to_owned(),
                };
                ProfileRow { name, immediate: false, loc, row }
            })
            .collect();
        let (t, j) = (self.stats.totals(), self.stats.jobs());
        let totals = ProfileTotals {
            evaluations: t.evaluations,
            steps: t.steps,
            wall_us: t.wall_us,
            trials: j.jobs.saturating_sub(t.evaluations),
            trial_steps: j.steps.saturating_sub(t.steps),
        };
        Some(ProfileRows { funcs, sites, totals })
    }

    /// The `-fconstexpr-stats` summary: one `constexpr: <what> key=value
    /// ...` line per counter group (spec/oven.md "Profile counters").
    pub fn stats_summary(&self) -> String {
        let t = self.stats.totals();
        let j = self.stats.jobs();
        let immediate =
            self.stats.records().iter().filter(|r| r.kind == EvalKind::Consteval).count();
        let (meta, compiles, compile_ns) =
            self.hooks.prof.as_ref().map_or((0, 0, 0), |p| (p.meta_ops, p.compiles, p.compile_ns));
        let (hits, misses) = (self.memo.hits, self.memo.misses);
        let mut s = String::new();
        let _ = writeln!(
            s,
            "constexpr: evaluations count={} failed={} immediate={} wall-us={}",
            t.evaluations, t.failed, immediate, t.wall_us
        );
        let _ = writeln!(s, "constexpr: steps count={}", t.steps);
        let _ = writeln!(
            s,
            "constexpr: calls count={} memo-hits={} memo-misses={} memo-hit-rate={}",
            t.calls,
            hits,
            misses,
            percent(hits, hits + misses)
        );
        let _ = writeln!(
            s,
            "constexpr: memory allocs={} bytes={} peak-live={} reclaimed={} peak-arena={}",
            t.allocs, t.bytes, t.peak_bytes, t.reclaimed_bytes, t.peak_arena
        );
        let _ = writeln!(
            s,
            "constexpr: interned frozen-bytes={} refl-bytes={} static-bytes={}",
            t.frozen_bytes, t.refl_bytes, t.static_bytes
        );
        let _ = writeln!(s, "constexpr: meta-ops count={meta} splice-fast={}", t.splice_fast);
        let trials = j.jobs.saturating_sub(t.evaluations);
        let _ = writeln!(
            s,
            "constexpr: trials count={} failed={} steps={} wall-us={}",
            trials,
            j.failed.saturating_sub(t.failed),
            j.steps.saturating_sub(t.steps),
            j.wall_us.saturating_sub(t.wall_us)
        );
        let _ =
            writeln!(s, "constexpr: compile functions={compiles} wall-us={}", compile_ns / 1000);
        s
    }
}

/// `n / d` as a percentage with one decimal (`-` when `d` is 0).
fn percent(n: u64, d: u64) -> String {
    if d == 0 {
        return "-".to_owned();
    }
    let tenths = (u128::from(n) * 1000 + u128::from(d) / 2) / u128::from(d);
    format!("{}.{}%", tenths / 10, tenths % 10)
}

/// Nanoseconds as milliseconds, for display.
#[allow(clippy::cast_precision_loss, reason = "display only; exact below 2^52 ns")]
fn ms(ns: u64) -> f64 {
    ns as f64 / 1e6
}

fn json_str(s: &str) -> String {
    let mut o = String::with_capacity(s.len() + 2);
    o.push('"');
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            c if (c as u32) < 0x20 => {
                let _ = write!(o, "\\u{:04x}", c as u32);
            }
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(name: &str, loc: u32, incl: u64, count: u64) -> ProfileRow {
        let row = Row { count, incl_steps: incl, self_steps: incl, ..Row::default() };
        ProfileRow { name: name.into(), immediate: false, loc: Loc::from_raw(loc), row }
    }

    fn locs(l: Loc) -> Option<(String, u32, u32)> {
        Some(("dir/t.cpp".into(), l.to_raw(), 1))
    }

    #[test]
    fn ranking_breaks_ties_by_count_then_location_then_name() {
        let rows = ProfileRows {
            funcs: vec![
                row("d", 9, 5, 1),
                row("c", 3, 5, 1),
                row("b", 3, 5, 1),
                row("a", 1, 5, 2),
                row("e", 1, 7, 1),
            ],
            sites: Vec::new(),
            totals: ProfileTotals::default(),
        };
        let opts = RenderOptions { json: true, limit: 0 };
        let out = rows.render("t.cpp", opts, &locs);
        let names: Vec<&str> = out
            .lines()
            .filter_map(|l| l.strip_prefix("{\"function\":\""))
            .map(|l| &l[..1])
            .collect();
        assert_eq!(names, ["e", "a", "b", "c", "d"]);
    }

    #[test]
    fn limit_and_text_shape() {
        let rows = ProfileRows {
            funcs: (0..5).map(|i| row("f", i, u64::from(i), 1)).collect(),
            sites: vec![row("static_assert", 1, 10, 1)],
            totals: ProfileTotals {
                evaluations: 1,
                steps: 10,
                wall_us: 2000,
                trials: 3,
                trial_steps: 4,
            },
        };
        let out = rows.render("t.cpp", RenderOptions { json: false, limit: 2 }, &locs);
        assert!(out.starts_with("===-- kiln constexpr profile: t.cpp --===\n"), "{out}");
        assert!(out.contains("functions (top 2 of 5, by inclusive steps):"), "{out}");
        assert!(out.contains("  ... 3 more rows (use -fconstexpr-profile-limit=0)"), "{out}");
        assert!(out.contains("(t.cpp:4:1)"), "{out}");
        assert!(
            out.contains(
                "  total: 1 evaluations, 10 steps, 2.000 ms; 3 trials, 4 steps; 5 calls\n"
            ),
            "{out}"
        );
    }

    #[test]
    fn recursion_charges_inclusive_once() {
        let mut p = Profile::new(Level::Calls);
        p.func_enter(0);
        p.func_enter(0);
        p.func_end(0, Spent { steps: 3, ..Spent::default() });
        p.func_end(0, Spent { steps: 10, child_steps: 3, ..Spent::default() });
        let r = p.funcs[0];
        assert_eq!((r.count, r.incl_steps, r.self_steps, r.active), (2, 10, 10, 0));
    }

    #[test]
    fn percentages() {
        assert_eq!(percent(1, 3), "33.3%");
        assert_eq!(percent(2, 3), "66.7%");
        assert_eq!(percent(0, 0), "-");
        assert_eq!(percent(5, 5), "100.0%");
    }

    #[test]
    fn json_escapes() {
        assert_eq!(json_str("a\"b\\c\n"), "\"a\\\"b\\\\c\\u000a\"");
    }
}
