//! One retry of a test that failed for want of file descriptors on the
//! host (`spec/harness.md` "Host errors", WP-2780).
//!
//! On a host that serves the checkout from a shared file server (a VM's
//! virtiofs share used by several machines' builds at once), opening a file
//! there fails in bursts with EMFILE, "Too many open files", in every
//! process at once (xtask reading a test, `sh` executing the compiler, the
//! compiler reading its input, `ld` creating its output), whatever the
//! process holds itself: the file server ran out, not the process. Such a
//! failure says nothing about the test, so a failed test whose output shows
//! it runs once more after [`PAUSE`]; a second host failure is the status
//! `HOST`, never the test's FAIL or ERROR, and the summary counts both.

use std::time::Duration;

use super::runner::{Status, TestResult};

/// How long a test waits before its second run: long enough for a burst
/// of other processes' opens to drain, short against a corpus run.
pub const PAUSE: Duration = Duration::from_secs(5);

/// Does `line` report a descriptor shortage? `strerror` of EMFILE ("Too many
/// open files") and of ENFILE ("Too many open files in system"), and Rust's
/// `os error 24` / `os error 23`.
pub fn is_descriptor_shortage(line: &str) -> bool {
    line.contains("Too many open files")
        || line.contains("(os error 24)")
        || line.contains("(os error 23)")
}

/// The first line of a failed result's details, reason or command log
/// that reports a descriptor shortage, trimmed and at most 300 bytes; `None`
/// for a result that is not a failure ([`Status::is_failure`]).
pub fn host_error(r: &TestResult) -> Option<String> {
    if !r.status.is_failure() {
        return None;
    }
    let reason = r.reason.iter().flat_map(|s| s.lines());
    let log = r.log.iter().flat_map(|s| s.lines());
    let line = r.details.lines().chain(reason).chain(log).find(|l| is_descriptor_shortage(l))?;
    let line = line.trim();
    let mut end = line.len().min(300);
    while !line.is_char_boundary(end) {
        end -= 1;
    }
    Some(line[..end].to_owned())
}

/// `first` as is when it shows no [host error](host_error); otherwise,
/// after `pause`, the result of `again` (the test's second run) with
/// [`TestResult::host_retry`] set, and with the status [`Status::Host`]
/// when the second run hit a host error too. The elapsed time covers both
/// runs and the pause.
pub fn retry_on_host_error(
    first: TestResult,
    pause: Duration,
    again: impl FnOnce() -> TestResult,
) -> TestResult {
    let Some(line) = host_error(&first) else { return first };
    std::thread::sleep(pause);
    let mut second = again();
    second.elapsed += first.elapsed + pause;
    if let Some(again_line) = host_error(&second) {
        second.details = format!(
            "the host ran out of file descriptors on both runs (no verdict on the test)\n  \
             first run: {line}\n  second run: {again_line}\n{}",
            second.details
        );
        second.status = Status::Host;
    }
    second.host_retry = Some(line);
    second
}

#[cfg(test)]
mod tests {
    use super::*;

    fn result(status: Status, details: &str, log: &[&str]) -> TestResult {
        TestResult {
            name: "tests/x.cpp".into(),
            status,
            reason: None,
            details: details.into(),
            log: log.iter().map(|&l| l.to_owned()).collect(),
            updated: false,
            advisory: None,
            gcc_compared: false,
            host_retry: None,
            elapsed: Duration::from_millis(10),
        }
    }

    /// The shapes seen in CI logs on a host whose file server ran out:
    /// xtask's own read, `sh` failing to exec the compiler, the compiler
    /// reading its input, the linker creating its output; ENFILE too.
    #[test]
    fn classifies_descriptor_shortages() {
        let host = [
            (Status::Error, "cannot read test: Too many open files (os error 24)", vec![]),
            (
                Status::Fail,
                "kiln rejected the test\n    kiln-mem-limit: 1: exec: /t/debug/kiln++: Too many open files",
                vec![],
            ),
            (Status::Fail, "", vec!["    error: error reading 'a.cpp': Too many open files"]),
            (
                Status::XPass,
                "/usr/bin/ld: cannot open output file t0/test.out: Too many open files",
                vec![],
            ),
            (
                Status::Error,
                "cannot create /t/run-1/t3: Too many open files in system (os error 23)",
                vec![],
            ),
            (Status::Killed, "x\ncannot read t.ll: (os error 24)", vec![]),
        ];
        for (status, details, log) in host {
            let r = result(status, details, &log);
            let line =
                host_error(&r).unwrap_or_else(|| panic!("not a host error: {details:?} {log:?}"));
            assert!(is_descriptor_shortage(&line) && line == line.trim(), "{line:?}");
        }
        // Not a failure, or no shortage: no host error.
        for (status, details) in [
            (Status::Pass, "cannot read test: Too many open files (os error 24)"),
            (Status::XFail, "Too many open files"),
            (Status::Skip, "Too many open files"),
            (Status::Fail, "expected diagnostic not seen: too many open braces"),
            (Status::Error, "cannot read test: No such file or directory (os error 2)"),
        ] {
            assert_eq!(host_error(&result(status, details, &[])), None, "{status:?} {details:?}");
        }
        // Long lines are cut at a character boundary.
        let long = format!("Too many open files {}", "é".repeat(400));
        let line = host_error(&result(Status::Fail, &long, &[])).unwrap();
        assert!(line.len() <= 300 && line.starts_with("Too many open files"));
    }

    #[test]
    fn retries_once_and_reports_the_host() {
        let emfile = "cannot read test: Too many open files (os error 24)";
        let pause = Duration::ZERO;
        // No host error: never run again.
        let r = retry_on_host_error(result(Status::Fail, "wrong output", &[]), pause, || {
            panic!("run again")
        });
        assert_eq!((r.status, r.host_retry), (Status::Fail, None));
        // A host error, then a pass: PASS, with the first run's line.
        let r = retry_on_host_error(result(Status::Error, emfile, &[]), pause, || {
            result(Status::Pass, "", &[])
        });
        assert_eq!((r.status, r.host_retry.as_deref()), (Status::Pass, Some(emfile)));
        assert_eq!(r.elapsed, Duration::from_millis(20));
        // A host error, then the test's own failure: that FAIL stands.
        let r = retry_on_host_error(result(Status::Error, emfile, &[]), pause, || {
            result(Status::Fail, "wrong output", &[])
        });
        assert_eq!((r.status, r.details.as_str()), (Status::Fail, "wrong output"));
        assert!(r.host_retry.is_some());
        // Twice: HOST, a failure of the run, never the test's ERROR.
        let r = retry_on_host_error(result(Status::Error, emfile, &[]), pause, || {
            result(
                Status::Fail,
                "",
                &["$ kiln++ a.cpp  # failed to start: Too many open files (os error 24)"],
            )
        });
        assert_eq!(r.status, Status::Host);
        assert!(r.status.is_failure() && r.status.label() == "HOST");
        assert!(
            r.details.starts_with("the host ran out of file descriptors on both runs"),
            "{}",
            r.details
        );
        assert!(r.details.contains("first run: cannot read test:"), "{}", r.details);
        assert!(r.details.contains("second run: $ kiln++ a.cpp"), "{}", r.details);
    }
}
