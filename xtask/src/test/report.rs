//! Console output of a test run (`spec/harness.md` "Output").

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::time::Duration;

use super::runner::{Status, TestResult, indent};

/// Per-status counts.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Counts(pub BTreeMap<Status, usize>);

impl Counts {
    pub fn of(results: &[TestResult]) -> Self {
        let mut m = BTreeMap::new();
        for r in results {
            *m.entry(r.status).or_insert(0) += 1;
        }
        Self(m)
    }

    pub fn get(&self, s: Status) -> usize {
        self.0.get(&s).copied().unwrap_or(0)
    }

    pub fn failed(&self) -> bool {
        self.0.iter().any(|(s, &n)| s.is_failure() && n > 0)
    }
}

/// The line(s) printed for one result. Passes are printed only when verbose;
/// skips and xfails always (they are tracked debt), with their reason.
pub fn format_result(r: &TestResult, verbose: bool) -> String {
    let mut s = String::new();
    if r.status == Status::Pass && !verbose {
        return s;
    }
    let _ = write!(s, "{:<6} {}", r.status.label(), r.name);
    if let Some(reason) = &r.reason {
        let _ = write!(s, " ({reason})");
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

/// The final summary line.
pub fn format_summary(counts: &Counts, total: usize, elapsed: Duration) -> String {
    let verdict = if counts.failed() { "FAILED" } else { "ok" };
    format!(
        "test result: {verdict}. {} passed; {} failed; {} xfail; {} xpass; {} skipped; {} errors; {total} tests in {:.2}s\n",
        counts.get(Status::Pass),
        counts.get(Status::Fail),
        counts.get(Status::XFail),
        counts.get(Status::XPass),
        counts.get(Status::Skip),
        counts.get(Status::Error),
        elapsed.as_secs_f64(),
    )
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
            "test result: ok. 1 passed; 0 failed; 1 xfail; 0 xpass; 1 skipped; 0 errors; 3 tests in 1.50s\n"
        );
        assert!(Counts::of(&[r(Status::XPass)]).failed());
        assert!(Counts::of(&[r(Status::Error)]).failed());
    }
}
