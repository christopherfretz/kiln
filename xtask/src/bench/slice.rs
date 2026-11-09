//! The `libcxx-slice` entry (`spec/bench.md` "The libcxx-slice entry"):
//! kiln against the comparison compiler at `-fsyntax-only` over a fixed
//! list of heavy libc++ tests, compiled with exactly the flags
//! `cargo xtask libcxx` gives them (`crate::libcxx::syntax_compiles`).
//!
//! Each listed test is a bench entry `libcxx-slice/<path without .cpp>`
//! measured like any other; this module reads the list, selects it, and
//! aggregates the per-test ratios (median, geometric mean, p90, max) for
//! the printed report and the `perf.md` section.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use super::compare::{self, RatioRow, ThresholdOverrides};
use super::entry::{BenchSpec, Entry, Mode};
use crate::util::{CompilerId, CompilerKind};

/// The group directory under the bench root, and the entry id prefix.
pub const GROUP: &str = "libcxx-slice";
/// The list file in [`GROUP`]'s directory.
pub const LIST: &str = "slice.list";
/// The libc++ release whose tests the slice compiles (the M4 gate's).
pub const TAG: &str = crate::libcxx::M4_TAG;
/// The heading of the slice's section of `perf.md`.
pub const HEADING: &str = "## libc++ slice";

/// The test paths of a list file: one path relative to `libcxx/test` per
/// line; blank lines and `#` comments are skipped. A path that is absolute,
/// climbs (`..`), does not end in `.cpp`, or repeats is an error, as is an
/// empty list.
pub fn parse_list(text: &str) -> Result<Vec<String>, String> {
    let mut out: Vec<String> = Vec::new();
    for (n, line) in text.lines().enumerate() {
        let line = line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        let at = |why: &str| format!("{LIST}:{}: {why} `{line}`", n + 1);
        if line.starts_with('/') || line.split('/').any(|c| c == ".." || c.is_empty()) {
            return Err(at("not a plain path relative to libcxx/test:"));
        }
        if Path::new(line).extension().is_none_or(|e| e != "cpp") {
            return Err(at("not a .cpp test:"));
        }
        if out.iter().any(|p| p == line) {
            return Err(at("listed twice:"));
        }
        out.push(line.to_owned());
    }
    if out.is_empty() {
        return Err(format!("{LIST}: no tests listed"));
    }
    Ok(out)
}

/// Splits the command-line inputs (canonical or not) into whether they
/// select the slice (its directory or its list file, named explicitly; the
/// bench root alone does not) and the other inputs.
pub fn split_inputs(bench_root: &Path, inputs: &[PathBuf]) -> (bool, Vec<PathBuf>) {
    let canon = |p: &Path| std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    let dir = canon(&bench_root.join(GROUP));
    let list = dir.join(LIST);
    let mut selected = false;
    let mut rest = Vec::new();
    for i in inputs {
        let c = canon(i);
        if c == dir || c == list {
            selected = true;
        } else {
            rest.push(i.clone());
        }
    }
    (selected, rest)
}

/// The oldest clang major the slice measures: libc++ 22's
/// `<__configuration/compiler.h>` warns "Libc++ only supports Clang 20 and
/// later" and the runner's `-Werror` makes that an error, so an older clang
/// fails every test and its ratios would mean nothing.
pub const MIN_CLANG_MAJOR: u32 = 20;

/// Refuses a clang that cannot be the slice's comparison point (WP-487):
/// one older than [`MIN_CLANG_MAJOR`], or whose `--version` names no
/// version. `role` is `--compare-with` or `--compiler`, `shown` the path as
/// given. kiln, GCC and other compilers pass (the floor is clang's).
pub fn check_clang_floor(id: &CompilerId, role: &str, shown: &str) -> Result<(), String> {
    if id.kind != CompilerKind::Clang {
        return Ok(());
    }
    let floor = MIN_CLANG_MAJOR;
    let why = match id.clang_version() {
        Some([major, ..]) if major >= floor => return Ok(()),
        Some([major, minor, patch]) => format!("is clang {major}.{minor}.{patch}"),
        None => "names no clang version".to_owned(),
    };
    Err(format!(
        "{role} `{shown}` {why} (`{}`): bench/{GROUP} needs clang {floor} or later, because \
         the libc++ headers of {TAG} reject older clangs (\"Libc++ only supports Clang {floor} and \
         later\" under the runner's -Werror), so every test would fail and the ratios would \
         compare nothing; use a clang {floor}+ such as `--compare-with clang++-22`",
        id.version
    ))
}

/// The entry id of a listed test.
pub fn id_of(rel: &str) -> String {
    format!("{GROUP}/{}", rel.strip_suffix(".cpp").unwrap_or(rel))
}

/// Whether an entry id belongs to the slice.
pub fn is_slice(id: &str) -> bool {
    id.strip_prefix(GROUP).is_some_and(|r| r.starts_with('/'))
}

/// The directives every slice entry has: `-fsyntax-only` (the libc++
/// runner adds `-std=` itself), default thresholds (never applied).
pub fn spec() -> BenchSpec {
    BenchSpec {
        mode: Mode::Syntax,
        std: "c++26".into(),
        args: Vec::new(),
        oven: Vec::new(),
        thresholds: ThresholdOverrides::default(),
        xfail_kiln: None,
        budget_time_ms: None,
        generate: None,
    }
}

/// The entries of the listed tests under `test_root` (`libcxx/test`),
/// keeping ids that contain `filter`.
pub fn entries(rels: &[String], test_root: &Path, filter: Option<&str>) -> Vec<Entry> {
    rels.iter()
        .map(|rel| {
            let id = id_of(rel);
            let path = test_root.join(rel);
            Entry {
                name: id[GROUP.len() + 1..].to_owned(),
                group: GROUP.to_owned(),
                arg: path.display().to_string(),
                path,
                id,
            }
        })
        .filter(|e| filter.is_none_or(|f| e.id.contains(f)))
        .collect()
}

/// The recorded `flags` of a slice compile: paths under `root` relative to
/// it, and toolchain paths from their `.toolchain/` component on, so a
/// worktree and the main checkout record the same string.
pub fn shown_flags(args: &[String], root: &Path) -> String {
    args.iter()
        .map(|a| {
            if let Ok(r) = Path::new(a).strip_prefix(root) {
                return r.display().to_string();
            }
            match a.find("/.toolchain/") {
                Some(at) if a.starts_with('/') => a[at + 1..].to_owned(),
                _ => a.clone(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// The distribution of one ratio over the measured tests.
#[derive(Clone, Debug, PartialEq)]
pub struct Dist {
    pub n: usize,
    pub median: f64,
    pub geomean: f64,
    /// Nearest-rank 90th percentile.
    pub p90: f64,
    pub max: f64,
    /// The entry id of the largest ratio (the first, on a tie).
    pub max_id: String,
}

impl Dist {
    /// `None` without a positive, finite ratio.
    pub fn of(ratios: &[(String, f64)]) -> Option<Self> {
        let mut v: Vec<&(String, f64)> =
            ratios.iter().filter(|(_, r)| *r > 0.0 && r.is_finite()).collect();
        if v.is_empty() {
            return None;
        }
        v.sort_by(|a, b| a.1.total_cmp(&b.1));
        let n = v.len();
        let median =
            if n % 2 == 1 { v[n / 2].1 } else { f64::midpoint(v[n / 2 - 1].1, v[n / 2].1) };
        // Nearest rank: the smallest value with at least 90% at or below it.
        let rank = (n * 9).div_ceil(10).max(1);
        let max = v[n - 1].1;
        // The first test with the maximum, in list order.
        let max_id = v.iter().find(|(_, r)| r.total_cmp(&max).is_eq()).map(|(id, _)| id.clone());
        let max_id = max_id.unwrap_or_default();
        let xs: Vec<f64> = v.iter().map(|(_, r)| *r).collect();
        Some(Self {
            n,
            median,
            geomean: compare::geomean(&xs).unwrap_or(median),
            p90: v[rank - 1].1,
            max,
            max_id,
        })
    }
}

/// The slice's aggregate over a run's ratio rows.
#[derive(Clone, Debug, PartialEq)]
pub struct Aggregate {
    /// Tests the run selected.
    pub tests: usize,
    /// Tests either compiler failed.
    pub failed: usize,
    pub time: Option<Dist>,
    pub mem: Option<Dist>,
}

impl Aggregate {
    pub fn of(rows: &[RatioRow]) -> Self {
        let pairs = |f: fn(&RatioRow) -> Option<f64>| -> Vec<(String, f64)> {
            rows.iter().filter_map(|r| Some((r.id.clone(), f(r)?))).collect()
        };
        Self {
            tests: rows.len(),
            failed: rows.iter().filter(|r| r.test.is_none() || r.oracle.is_none()).count(),
            time: Dist::of(&pairs(RatioRow::time_ratio)),
            mem: Dist::of(&pairs(RatioRow::mem_ratio)),
        }
    }

    /// The two aggregate lines, as `<label>: median ..x, geomean ..x, p90
    /// ..x, max ..x (<id>) over N tests`.
    pub fn lines(&self, t: &str, o: &str) -> Vec<String> {
        let line = |what: &str, d: Option<&Dist>| match d {
            None => format!("{t}/{o} {what}: n/a"),
            Some(d) => format!(
                "{t}/{o} {what}: median {:.2}x, geomean {:.2}x, p90 {:.2}x, max {:.2}x ({}) \
                 over {} tests",
                d.median,
                d.geomean,
                d.p90,
                d.max,
                d.max_id.strip_prefix(&format!("{GROUP}/")).unwrap_or(&d.max_id),
                d.n
            ),
        };
        vec![line("time", self.time.as_ref()), line("memory", self.mem.as_ref())]
    }
}

/// The 1-minute load averages sampled before each slice test.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Loads(pub Vec<f64>);

impl Loads {
    /// `first X, mean Y, max Z`, or `unknown`.
    #[allow(clippy::cast_precision_loss)]
    pub fn describe(&self) -> String {
        let v = &self.0;
        if v.is_empty() {
            return "unknown".into();
        }
        let mean = v.iter().sum::<f64>() / v.len() as f64;
        let max = v.iter().copied().fold(f64::MIN, f64::max);
        format!("first {:.1}, mean {mean:.1}, max {max:.1}", v[0])
    }
}

/// The printed report: the per-test ratio table and the aggregate lines.
pub fn render_text(rows: &[RatioRow], t: &str, o: &str, loads: &Loads) -> String {
    let mut out = String::new();
    let h = [
        "test".to_owned(),
        format!("{t} ms"),
        format!("{o} ms"),
        format!("{t}/{o} time"),
        format!("{t} MB"),
        format!("{o} MB"),
        format!("{t}/{o} mem"),
    ];
    let hdr: Vec<&str> = h.iter().map(String::as_str).collect();
    let cells: Vec<Vec<String>> = rows.iter().map(cells).collect();
    out.push_str(&compare::render_table(&hdr, &cells));
    let agg = Aggregate::of(rows);
    let _ = writeln!(
        out,
        "{GROUP}: {} tests, {} failed; load average (1 min) {}",
        agg.tests,
        agg.failed,
        loads.describe()
    );
    for l in agg.lines(t, o) {
        let _ = writeln!(out, "{l}");
    }
    out
}

/// A per-test row: the test path (without the group), both medians, the
/// time ratio, both peak RSS values and the memory ratio.
fn cells(r: &RatioRow) -> Vec<String> {
    let mut c = r.cells();
    c.truncate(8);
    c.remove(1); // mode: always syntax
    if let Some(id) = c[0].strip_prefix(&format!("{GROUP}/")).map(str::to_owned) {
        c[0] = id;
    }
    c
}

/// What the `perf.md` section says about its recording.
#[derive(Debug)]
pub struct SectionContext<'a> {
    pub when: &'a str,
    pub stamp: &'a str,
    /// (path, kind, version) of the compiler under test and of the oracle.
    pub test: (&'a str, &'a str, &'a str),
    pub oracle: (&'a str, &'a str, &'a str),
    pub host: String,
    pub runs: usize,
    pub rss: &'a str,
    pub loads: &'a Loads,
    /// Tests in the list (the run may have selected fewer).
    pub listed: usize,
}

/// The slice's `perf.md` section, from [`HEADING`] on, ending in a blank
/// line.
pub fn render_section(rows: &[RatioRow], cx: &SectionContext<'_>) -> String {
    let (t, o) = compare::ratio_labels(cx.test.1, cx.oracle.1);
    let agg = Aggregate::of(rows);
    let mut s = String::new();
    let _ = writeln!(s, "{HEADING}\n");
    let _ = writeln!(
        s,
        "Generated by `cargo xtask bench --compare-with {} --record bench/{GROUP}` on {} \
         (results `{}`); every recording that selects the slice rewrites this section, and \
         other recordings keep it. {} of the {} tests of `bench/{GROUP}/{LIST}` ({TAG} \
         `libcxx/test`), `-fsyntax-only` with the flags `cargo xtask libcxx` gives each test, \
         one compile at a time, {} runs each (median after dropping the fastest and slowest). \
         Record-only: the ratios move with the box load, so they are not judged \
         (spec/bench.md \"The libcxx-slice entry\").\n",
        cx.oracle.0, cx.when, cx.stamp, agg.tests, cx.listed, cx.runs
    );
    let _ = writeln!(s, "- {t}: `{}` ({})", cx.test.0, cx.test.2);
    let _ = writeln!(s, "- {o}: `{}` ({})", cx.oracle.0, cx.oracle.2);
    let _ = writeln!(s, "- Host: {}; memory is the peak RSS ({})", cx.host, cx.rss);
    let _ = writeln!(s, "- Load average (1 min, before each test): {}\n", cx.loads.describe());
    let _ = writeln!(s, "| Ratio | median | geomean | p90 | max | tests |");
    let _ = writeln!(s, "|---|--:|--:|--:|---|--:|");
    for (what, d) in [("time", &agg.time), ("memory", &agg.mem)] {
        match d {
            Some(d) => {
                let max_id = d.max_id.strip_prefix(&format!("{GROUP}/")).unwrap_or(&d.max_id);
                let _ = writeln!(
                    s,
                    "| {t}/{o} {what} | {:.2}x | {:.2}x | {:.2}x | {:.2}x `{max_id}` | {} |",
                    d.median, d.geomean, d.p90, d.max, d.n
                );
            }
            None => {
                let _ = writeln!(s, "| {t}/{o} {what} | - | - | - | - | 0 |");
            }
        }
    }
    if agg.failed > 0 {
        let _ = writeln!(s, "\n{} test(s) failed with either compiler (FAIL below).", agg.failed);
    }
    let _ = writeln!(
        s,
        "\n| Test | {t} ms | {o} ms | {t}/{o} time | {t} MB | {o} MB | {t}/{o} memory |"
    );
    let _ = writeln!(s, "|---|--:|--:|--:|--:|--:|--:|");
    for r in rows {
        let mut c = cells(r);
        c[0] = format!("`{}`", c[0]);
        let _ = writeln!(s, "| {} |", c.join(" | "));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::super::record::tests::sample;
    use super::*;

    fn row(id: &str, t: Option<(f64, u64)>, o: Option<(f64, u64)>) -> RatioRow {
        RatioRow {
            id: id_of(id),
            mode: "syntax".into(),
            test: t.map(|(ms, kb)| sample("kiln++", ms, Some(kb))),
            oracle: o.map(|(ms, kb)| sample("clang++-22", ms, Some(kb))),
        }
    }

    #[test]
    fn parses_the_list() {
        let text = "# heavy tests\n\nstd/a/b.pass.cpp\n  std/c.compile.pass.cpp  # why\n";
        assert_eq!(parse_list(text).unwrap(), ["std/a/b.pass.cpp", "std/c.compile.pass.cpp"]);
        for (bad, why) in [
            ("/abs/x.pass.cpp", "not a plain path"),
            ("std/../x.pass.cpp", "not a plain path"),
            ("std//x.pass.cpp", "not a plain path"),
            ("std/x.h", "not a .cpp test"),
            ("std/x.pass.cpp\nstd/x.pass.cpp", "listed twice"),
        ] {
            let e = parse_list(bad).unwrap_err();
            assert!(e.contains(why), "{bad}: {e}");
        }
        assert!(parse_list("# nothing\n").unwrap_err().contains("no tests"));
    }

    #[test]
    fn the_committed_list_parses() {
        let root = crate::util::repo_root();
        let text = std::fs::read_to_string(root.join("bench").join(GROUP).join(LIST)).unwrap();
        let rels = parse_list(&text).unwrap();
        assert!(rels.len() >= 40, "{} tests", rels.len());
        assert!(rels.windows(2).all(|w| w[0] < w[1]), "the list is sorted");
    }

    #[test]
    fn selection_is_explicit() {
        let root = crate::util::repo_root();
        let bench = root.join("bench");
        let dir = bench.join(GROUP);
        let (on, rest) = split_inputs(&bench, std::slice::from_ref(&bench));
        assert!(!on);
        assert_eq!(rest, std::slice::from_ref(&bench));
        let other = bench.join("oven");
        let (on, rest) = split_inputs(&bench, &[dir.clone(), other.clone()]);
        assert!(on);
        assert_eq!(rest, [other]);
        let (on, rest) = split_inputs(&bench, &[dir.join(LIST)]);
        assert!(on && rest.is_empty());
    }

    #[test]
    fn ids_and_entries() {
        assert_eq!(id_of("std/a/b.pass.cpp"), "libcxx-slice/std/a/b.pass");
        assert!(is_slice("libcxx-slice/std/a"));
        assert!(!is_slice("libcxx-slicex/a") && !is_slice("oven/fib"));
        let rels = vec!["std/a/b.pass.cpp".to_owned(), "std/c/d.pass.cpp".to_owned()];
        let e = entries(&rels, Path::new("/t"), Some("c/d"));
        assert_eq!(e.len(), 1);
        assert_eq!(e[0].id, "libcxx-slice/std/c/d.pass");
        assert_eq!(e[0].group, GROUP);
        assert_eq!(e[0].name, "std/c/d.pass");
        assert_eq!(e[0].path, Path::new("/t/std/c/d.pass.cpp"));
    }

    #[test]
    fn shows_flags_without_checkout_paths() {
        let args: Vec<String> = [
            "-resource-dir",
            "/w/tree/resource",
            "-isystem",
            "/m/kiln/.toolchain/libcxx-22-root/usr/include",
            "-I",
            "rel/.toolchain/x",
            "-fsyntax-only",
        ]
        .map(str::to_owned)
        .into();
        assert_eq!(
            shown_flags(&args, Path::new("/w/tree")),
            "-resource-dir resource -isystem .toolchain/libcxx-22-root/usr/include -I \
             rel/.toolchain/x -fsyntax-only"
        );
    }

    #[test]
    fn distributions() {
        let r = |v: &[f64]| -> Vec<(String, f64)> {
            v.iter().enumerate().map(|(i, x)| (format!("t{i}"), *x)).collect()
        };
        let d = Dist::of(&r(&[2.0, 1.0, 4.0])).unwrap();
        assert_eq!((d.n, d.median, d.p90, d.max), (3, 2.0, 4.0, 4.0));
        assert_eq!(d.max_id, "t2");
        assert!((d.geomean - 2.0).abs() < 1e-12);
        // Even count: the mean of the middle two. Nearest-rank p90 of 10
        // values is the 9th.
        let ten: Vec<f64> = (1..=10).map(f64::from).collect();
        let d = Dist::of(&r(&ten)).unwrap();
        assert_eq!((d.median, d.p90, d.max), (5.5, 9.0, 10.0));
        // One value; zero and non-finite ratios are left out.
        let d = Dist::of(&r(&[0.0, 1.5, f64::NAN])).unwrap();
        assert_eq!((d.n, d.median, d.p90, d.max), (1, 1.5, 1.5, 1.5));
        assert_eq!(d.max_id, "t1");
        assert!(Dist::of(&r(&[0.0])).is_none());
        // A tie for the maximum names the first test.
        assert_eq!(Dist::of(&r(&[3.0, 1.0, 3.0])).unwrap().max_id, "t0");
    }

    #[test]
    fn aggregates_rows() {
        let rows = vec![
            row("std/a.pass.cpp", Some((100.0, 1024)), Some((100.0, 4096))),
            row("std/b.pass.cpp", Some((300.0, 2048)), Some((100.0, 4096))),
            row("std/c.pass.cpp", None, Some((100.0, 4096))),
            row("std/d.pass.cpp", Some((200.0, 1024)), Some((100.0, 1024))),
        ];
        let a = Aggregate::of(&rows);
        assert_eq!((a.tests, a.failed), (4, 1));
        let t = a.time.clone().unwrap();
        assert_eq!((t.n, t.median, t.p90, t.max), (3, 2.0, 3.0, 3.0));
        assert_eq!(t.max_id, "libcxx-slice/std/b.pass");
        let m = a.mem.clone().unwrap();
        assert_eq!((m.n, m.median, m.max), (3, 0.5, 1.0));
        assert_eq!(
            a.lines("kiln", "clang")[0],
            "kiln/clang time: median 2.00x, geomean 1.82x, p90 3.00x, max 3.00x \
             (std/b.pass) over 3 tests"
        );
        let text = render_text(&rows, "kiln", "clang", &Loads(vec![10.0, 14.0]));
        assert!(text.contains("libcxx-slice: 4 tests, 1 failed; load average (1 min) first 10.0, mean 12.0, max 14.0"), "{text}");
        assert!(text.contains("std/c.pass") && text.contains("FAIL"), "{text}");
        let loads = Loads(vec![12.5]);
        let section = render_section(
            &rows,
            &SectionContext {
                when: "2026-11-03 12:00:00 UTC",
                stamp: "2026-11-03T120000",
                test: ("kiln++", "kiln", "kiln 0.0.1"),
                oracle: ("clang++-22", "clang", "clang 22"),
                host: "aarch64, 24 CPUs".into(),
                runs: 3,
                rss: "gnu-time",
                loads: &loads,
                listed: 48,
            },
        );
        assert!(section.starts_with(HEADING), "{section}");
        assert!(section.contains("4 of the 48 tests"), "{section}");
        assert!(
            section
                .contains("| kiln/clang time | 2.00x | 1.82x | 3.00x | 3.00x `std/b.pass` | 3 |"),
            "{section}"
        );
        assert!(section.contains("| `std/c.pass` | FAIL | 100.0 | - | - | 4.0 | - |"), "{section}");
        assert!(section.contains("first 12.5, mean 12.5, max 12.5"), "{section}");
        assert!(section.ends_with("|\n"), "{section}");
    }

    /// The oracle floor (WP-487): clang 20+ passes, clang 19 and a clang
    /// with no readable version are refused with the reason, other kinds pass.
    #[test]
    fn refuses_clang_older_than_20() {
        let id = |kind, v: &str| CompilerId { kind, version: v.into() };
        let ok = |v| check_clang_floor(&id(CompilerKind::Clang, v), "--compare-with", "cc");
        assert!(ok("Debian clang version 20.1.8 (1)").is_ok());
        assert!(ok("Ubuntu clang version 22.1.0").is_ok());
        assert!(ok("clang version 23.0.0git (https://x y)").is_ok());
        let e = ok("Debian clang version 19.1.7 (3+b1)").unwrap_err();
        assert!(e.starts_with("--compare-with `cc` is clang 19.1.7 (`Debian clang"), "{e}");
        assert!(e.contains("needs clang 20 or later") && e.contains("clang++-22"), "{e}");
        let e = ok("clang version unknown").unwrap_err();
        assert!(e.contains("names no clang version"), "{e}");
        for kind in [CompilerKind::Kiln, CompilerKind::Gcc, CompilerKind::Other] {
            assert!(check_clang_floor(&id(kind, "x version 1"), "--compiler", "x").is_ok());
        }
    }

    #[test]
    fn help_lists_the_entry() {
        let usage = super::super::USAGE;
        assert!(usage.contains("bench/libcxx-slice") && usage.contains(LIST), "{usage}");
    }

    #[test]
    fn loads() {
        assert_eq!(Loads::default().describe(), "unknown");
    }
}
