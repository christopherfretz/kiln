//! Console output of a test run (`spec/harness.md` "Output").

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;
use std::time::{Duration, SystemTime};

use super::directives::PpCompare;
use super::runner::{Status, TestResult, indent};
use crate::bench::record::human_time;

/// Per-status counts, how many results are advisory (the GCC oracle's
/// comparison was dropped; `spec/harness.md` "GCC oracle"), and how many
/// were compared with the GCC oracle.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Counts(pub BTreeMap<Status, usize>, pub usize, pub usize);

impl Counts {
    pub fn of(results: &[TestResult]) -> Self {
        let mut m = BTreeMap::new();
        for r in results {
            *m.entry(r.status).or_insert(0) += 1;
        }
        Self(
            m,
            results.iter().filter(|r| r.advisory.is_some()).count(),
            results.iter().filter(|r| r.gcc_compared).count(),
        )
    }

    /// Results the GCC oracle's verdict or output was compared with.
    pub fn gcc_compared(&self) -> usize {
        self.2
    }

    /// Results whose GCC comparison was dropped (never a failure).
    pub fn advisory(&self) -> usize {
        self.1
    }

    pub fn get(&self, s: Status) -> usize {
        self.0.get(&s).copied().unwrap_or(0)
    }

    pub fn failed(&self) -> bool {
        self.0.iter().any(|(s, &n)| s.is_failure() && n > 0)
    }
}

/// The line(s) printed for one result. Passes are printed only when verbose
/// or advisory; skips and xfails always (they are tracked debt), with their
/// reason.
pub fn format_result(r: &TestResult, verbose: bool) -> String {
    let mut s = String::new();
    if r.status == Status::Pass && !verbose && r.advisory.is_none() {
        return s;
    }
    let _ = write!(s, "{:<6} {}", r.status.label(), r.name);
    if let Some(reason) = &r.reason {
        let _ = write!(s, " ({reason})");
    }
    if let Some(a) = &r.advisory {
        let _ = write!(s, " [{a}]");
    }
    if r.updated {
        s.push_str(" [updated .expected]");
    }
    if verbose {
        let _ = write!(s, " [{:.2}s]", r.elapsed.as_secs_f64());
    }
    s.push('\n');
    let show_details = r.status.is_failure() || (verbose && r.status == Status::XFail);
    if show_details && !r.details.is_empty() {
        s.push_str(&indent(&r.details));
    }
    if verbose || r.status.is_failure() {
        for cmd in &r.log {
            let _ = writeln!(s, "    {cmd}");
        }
    }
    s
}

/// `--pch-diff`'s accounting (`spec/harness.md` "PCH differential"),
/// printed after the summary: of the tests whose consumer ran, how many
/// took each path (under `snapshot`: restored, or replayed and why, with
/// [`crate::util::pch_diff::replay_reason`]'s grouping), how many of
/// those failed, and the skips by reason.
pub fn format_pch_accounting(
    mode: crate::util::pch_diff::PchDiff,
    results: &[TestResult],
) -> String {
    use crate::util::pch_diff::{RESTORED, replay_reason};
    let mut paths: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    let mut skips: BTreeMap<String, usize> = BTreeMap::new();
    let mut unreached = 0;
    for r in results {
        match (r.status, &r.reason) {
            (Status::Skip, why) => {
                *skips.entry(skip_key(why.as_deref().unwrap_or("(no reason)"))).or_default() += 1;
            }
            (Status::Pass | Status::Fail | Status::Error, Some(p))
                if r.status == Status::Pass || is_pch_path(p) =>
            {
                let key = replay_reason(p).map_or_else(|| p.clone(), |w| format!("replay: {w}"));
                let e = paths.entry(key).or_default();
                e.0 += 1;
                e.1 += usize::from(r.status != Status::Pass);
            }
            _ => unreached += 1,
        }
    }
    let ran: usize = paths.values().map(|v| v.0).sum();
    let skipped: usize = skips.values().sum();
    let mut s = format!("pch-diff {}: {ran} consumers ran", mode.as_str());
    if mode == crate::util::pch_diff::PchDiff::Snapshot {
        let restored = paths.get(RESTORED).map_or(0, |v| v.0);
        let _ = write!(s, ": {restored} restored, {} replayed", ran - restored);
    }
    let _ = writeln!(s, "; {unreached} failed before the consumer ran; {skipped} skipped");
    for (p, (n, failed)) in &paths {
        let f = if *failed > 0 { format!(" ({failed} failed)") } else { String::new() };
        let _ = writeln!(s, "  {n:>5}  {p}{f}");
    }
    if !skips.is_empty() {
        let _ = writeln!(s, "  skipped:");
        for (why, n) in &skips {
            let _ = writeln!(s, "  {n:>5}  {why}");
        }
    }
    s
}

/// Whether `reason` is a PCH path (not a skip's or a killed test's reason).
fn is_pch_path(reason: &str) -> bool {
    reason == crate::util::pch_diff::RESTORED || reason.starts_with("replay")
}

/// A skip reason reduced for [`format_pch_accounting`]'s histogram: its
/// first line, without the `pch-diff: ` prefix, cut after `kiln fails
/// without a PCH` (whose detail is the failure), numbers as `N`.
fn skip_key(why: &str) -> String {
    let first = why.lines().next().unwrap_or("");
    let first = first.strip_prefix("pch-diff: ").unwrap_or(first);
    for cut in ["kiln fails without a PCH", "requires: "] {
        if first.starts_with(cut) {
            return cut.trim_end_matches(": ").to_owned();
        }
    }
    let mut out = String::new();
    let mut digits = false;
    for c in first.chars() {
        if c.is_ascii_digit() {
            if !digits {
                out.push('N');
            }
            digits = true;
        } else {
            digits = false;
            out.push(c);
        }
    }
    out
}

/// The final summary line; `A advisory; ` appears before the total only
/// when a GCC comparison was dropped.
pub fn format_summary(counts: &Counts, total: usize, elapsed: Duration) -> String {
    let verdict = if counts.failed() { "FAILED" } else { "ok" };
    let gcc = match counts.gcc_compared() {
        0 => String::new(),
        n => format!("{n} compared with gcc; "),
    };
    let advisory = match counts.advisory() {
        0 => String::new(),
        n => format!("{n} advisory; "),
    };
    format!(
        "test result: {verdict}. {} passed; {} failed; {} xfail; {} xpass; {} skipped; {} errors; {} killed; {gcc}{advisory}{total} tests in {:.2}s\n",
        counts.get(Status::Pass),
        counts.get(Status::Fail),
        counts.get(Status::XFail),
        counts.get(Status::XPass),
        counts.get(Status::Skip),
        counts.get(Status::Error),
        counts.get(Status::Killed),
        elapsed.as_secs_f64(),
    )
}

/// The outcome of one corpus run of `cargo xtask ci` (`spec/harness.md`
/// "CI"), as far as the gate is concerned.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CorpusOutcome {
    /// The run was not made (only the clang self-check may be skipped:
    /// `--quick`, or clang 19 is not installed), with the reason.
    Skipped(String),
    /// The run finished: its per-status counts, test count and duration.
    Ran { counts: Counts, total: usize, elapsed: Duration },
    /// An infrastructure error (`cargo xtask test` exit status 2).
    Error(String),
}

impl CorpusOutcome {
    /// The outcome of [`super::execute`]'s result.
    pub fn of(run: &Result<Vec<TestResult>, String>, elapsed: Duration) -> Self {
        match run {
            Ok(results) => Self::Ran { counts: Counts::of(results), total: results.len(), elapsed },
            Err(e) => Self::Error(e.clone()),
        }
    }

    /// Does this run let `ci` pass? A run passes when it has no FAIL, XPASS,
    /// ERROR or KILLED (XFAIL and SKIP are allowed); a skipped run passes (the
    /// caller decides which runs may be skipped); an infrastructure error
    /// fails.
    pub fn passes(&self) -> bool {
        match self {
            Self::Skipped(_) => true,
            Self::Ran { counts, .. } => !counts.failed(),
            Self::Error(_) => false,
        }
    }

    /// The one-line result `ci` prints for this run.
    pub fn line(&self) -> String {
        match self {
            Self::Skipped(why) => format!("skipped ({why})"),
            Self::Ran { counts, total, elapsed } => {
                format_summary(counts, *total, *elapsed).trim_end().to_owned()
            }
            Self::Error(e) => format!("error: {e}"),
        }
    }
}

/// Should `cargo xtask ci` rerun a cargo step that failed with `stderr`
/// (`spec/harness.md` "CI")? Once, and only for rustc's `error[E0463]`
/// (can't find crate): a dependency artifact went missing under the build,
/// as when another cargo process shares the target directory; a rerun
/// rebuilds it. Every other failure is real.
pub fn ci_should_retry(stderr: &str, retried: bool) -> bool {
    !retried && stderr.lines().any(|l| l.trim_start().starts_with("error[E0463]"))
}

/// `cargo xtask ci`'s corpus verdict: it passes only if every run passes.
pub fn ci_corpus_passes(runs: &[CorpusOutcome]) -> bool {
    runs.iter().all(CorpusOutcome::passes)
}

/// The start and end markers of the generated region of a parity report.
pub const PARITY_BEGIN: &str = "<!-- parity:begin -->";
pub const PARITY_END: &str = "<!-- parity:end -->";

/// What a parity report says about the run that produced it.
#[derive(Clone, Debug)]
pub struct ParityHeader {
    /// The compiler's `--version` line.
    pub compiler: String,
    pub oracle: Option<String>,
    pub pp_compare: Option<PpCompare>,
    pub date: SystemTime,
    pub elapsed: Duration,
}

/// The generated region of a parity report (`--report-parity`): one row of
/// counts per corpus directory (the directory containing the test), sorted,
/// plus a total.
pub fn parity_table(h: &ParityHeader, results: &[TestResult]) -> String {
    let mut dirs: BTreeMap<&str, Vec<&TestResult>> = BTreeMap::new();
    for r in results {
        let dir = r.name.rsplit_once('/').map_or(".", |(d, _)| d);
        dirs.entry(dir).or_default().push(r);
    }
    let mut s = String::new();
    let _ = writeln!(s, "Generated by `cargo xtask test --report-parity`; do not edit by hand.\n");
    let _ = writeln!(s, "- compiler: `{}`", h.compiler);
    let _ = writeln!(
        s,
        "- oracle: {}",
        h.oracle.as_deref().map_or("none".to_owned(), |o| format!("`{o}`"))
    );
    let rule = match h.pp_compare {
        None => "per test (`pp-compare:`, default tokens)",
        Some(PpCompare::Tokens) => "tokens (forced by `--pp-compare`)",
        Some(PpCompare::Text) => "text (forced by `--pp-compare`)",
    };
    let _ = writeln!(s, "- pp comparison: {rule}");
    let _ = writeln!(s, "- run: {}, {:.1}s\n", human_time(h.date), h.elapsed.as_secs_f64());
    s.push_str("| directory | tests | pass | fail | xfail | xpass | skip | error | pass rate |\n");
    s.push_str("|---|---:|---:|---:|---:|---:|---:|---:|---:|\n");
    let row = |s: &mut String, name: &str, rs: &[&TestResult]| {
        let mut c = Counts::default();
        for r in rs {
            *c.0.entry(r.status).or_insert(0) += 1;
            c.1 += usize::from(r.advisory.is_some());
        }
        // Skipped tests are not counted against the rate.
        let ran = rs.len() - c.get(Status::Skip);
        // Permille in integers: counts are far below f64's exact range, but
        // the lint cannot know that. No test ran: no rate.
        let rate = (c.get(Status::Pass) * 1000 + ran / 2)
            .checked_div(ran)
            .map_or_else(|| "-".to_owned(), |p| format!("{}.{}%", p / 10, p % 10));
        let _ = writeln!(
            s,
            "| {name} | {} | {} | {} | {} | {} | {} | {} | {rate} |",
            rs.len(),
            c.get(Status::Pass),
            // A memory-limit kill is a failure of the compiler (or of the
            // test binary) for the parity table.
            c.get(Status::Fail) + c.get(Status::Killed),
            c.get(Status::XFail),
            c.get(Status::XPass),
            c.get(Status::Skip),
            c.get(Status::Error),
        );
    };
    for (dir, rs) in &dirs {
        row(&mut s, &format!("`{dir}`"), rs);
    }
    let all: Vec<&TestResult> = results.iter().collect();
    row(&mut s, "**total**", &all);
    s
}

/// Replaces the region between [`PARITY_BEGIN`] and [`PARITY_END`] in
/// `existing` (appending the markers when absent), keeping everything else.
pub fn splice_parity(existing: &str, table: &str) -> Result<String, String> {
    let region = format!("{PARITY_BEGIN}\n{table}{PARITY_END}");
    match (existing.find(PARITY_BEGIN), existing.find(PARITY_END)) {
        (Some(b), Some(e)) if b < e => {
            Ok(format!("{}{region}{}", &existing[..b], &existing[e + PARITY_END.len()..]))
        }
        (None, None) => {
            let sep = if existing.is_empty() || existing.ends_with("\n\n") {
                ""
            } else if existing.ends_with('\n') {
                "\n"
            } else {
                "\n\n"
            };
            Ok(format!("{existing}{sep}{region}\n"))
        }
        _ => Err(format!("mismatched `{PARITY_BEGIN}`/`{PARITY_END}` markers")),
    }
}

/// Writes the parity report for `results` into `path`.
pub fn write_parity(path: &Path, h: &ParityHeader, results: &[TestResult]) -> Result<(), String> {
    let existing = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => "# Parity report\n\n".to_owned(),
        Err(e) => return Err(format!("cannot read {}: {e}", path.display())),
    };
    let new = splice_parity(&existing, &parity_table(h, results))
        .map_err(|e| format!("{}: {e}", path.display()))?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)
            .map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    }
    std::fs::write(path, new).map_err(|e| format!("cannot write {}: {e}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(status: Status) -> TestResult {
        TestResult {
            name: "tests/x.cpp".into(),
            status,
            reason: None,
            details: "why".into(),
            log: vec!["$ cc x.cpp  # exit code 1".into()],
            updated: false,
            advisory: None,
            gcc_compared: false,
            elapsed: Duration::ZERO,
        }
    }

    #[test]
    fn formatting() {
        assert_eq!(format_result(&r(Status::Pass), false), "");
        assert_eq!(
            format_result(&r(Status::Fail), false),
            "FAIL   tests/x.cpp\n    why\n    $ cc x.cpp  # exit code 1\n"
        );
        let mut s = r(Status::Skip);
        s.reason = Some("because".into());
        assert_eq!(format_result(&s, false), "SKIP   tests/x.cpp (because)\n");
        let results = [r(Status::Pass), r(Status::XFail), r(Status::Skip)];
        let c = Counts::of(&results);
        assert!(!c.failed());
        assert_eq!(
            format_summary(&c, 3, Duration::from_millis(1500)),
            "test result: ok. 1 passed; 0 failed; 1 xfail; 0 xpass; 1 skipped; 0 errors; 0 killed; 3 tests in 1.50s\n"
        );
        let mut a = r(Status::Pass);
        a.advisory = Some("advisory: oracle crashed (internal compiler error)".into());
        assert_eq!(
            format_result(&a, false),
            "PASS   tests/x.cpp [advisory: oracle crashed (internal compiler error)]\n"
        );
        let c = Counts::of(&[a.clone(), r(Status::Pass)]);
        assert!(!c.failed() && c.advisory() == 1);
        assert_eq!(
            format_summary(&c, 2, Duration::from_millis(10)),
            "test result: ok. 2 passed; 0 failed; 0 xfail; 0 xpass; 0 skipped; 0 errors; 0 killed; 1 advisory; 2 tests in 0.01s\n"
        );
        let mut g = r(Status::Pass);
        g.gcc_compared = true;
        let c = Counts::of(&[g.clone(), g, a]);
        assert_eq!((c.gcc_compared(), c.advisory()), (2, 1));
        assert_eq!(
            format_summary(&c, 3, Duration::from_millis(10)),
            "test result: ok. 3 passed; 0 failed; 0 xfail; 0 xpass; 0 skipped; 0 errors; 0 killed; 2 compared with gcc; 1 advisory; 3 tests in 0.01s\n"
        );
        assert!(Counts::of(&[r(Status::XPass)]).failed());
        assert!(Counts::of(&[r(Status::Error)]).failed());
    }

    #[test]
    fn parity_report() {
        let mk = |name: &str, status| TestResult { name: name.into(), ..r(status) };
        let results = [
            mk("tests/differential/pp/libcxx/vector.cpp", Status::Pass),
            mk("tests/differential/pp/libcxx/map.cpp", Status::Fail),
            mk("tests/differential/pp/torture/a.cpp", Status::Pass),
            mk("tests/differential/pp/torture/b.cpp", Status::Skip),
        ];
        let h = ParityHeader {
            compiler: "kiln version 0.1".into(),
            oracle: Some("clang++-19".into()),
            pp_compare: None,
            date: std::time::UNIX_EPOCH,
            elapsed: Duration::from_millis(2500),
        };
        let t = parity_table(&h, &results);
        assert!(t.contains("- compiler: `kiln version 0.1`\n- oracle: `clang++-19`\n"), "{t}");
        assert!(t.contains("- run: 1970-01-01 00:00:00 UTC, 2.5s\n"), "{t}");
        assert!(t.contains("| `tests/differential/pp/libcxx` | 2 | 1 | 1 | 0 | 0 | 0 | 0 | 50.0% |\n| `tests/differential/pp/torture` | 2 | 1 | 0 | 0 | 0 | 1 | 0 | 100.0% |\n"), "{t}");
        assert!(t.ends_with("| **total** | 4 | 2 | 1 | 0 | 0 | 1 | 0 | 66.7% |\n"), "{t}");

        let doc = "# T\n\nintro\n<!-- parity:begin -->\nold\n<!-- parity:end -->\n\nfooter\n";
        assert_eq!(
            splice_parity(doc, "new\n").unwrap(),
            "# T\n\nintro\n<!-- parity:begin -->\nnew\n<!-- parity:end -->\n\nfooter\n"
        );
        assert_eq!(
            splice_parity("# T\n", "x\n").unwrap(),
            "# T\n\n<!-- parity:begin -->\nx\n<!-- parity:end -->\n"
        );
        assert!(splice_parity("<!-- parity:end --><!-- parity:begin -->", "x").is_err());
    }
}
