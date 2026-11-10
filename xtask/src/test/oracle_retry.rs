//! One retry of a clang oracle compile that died of a signal
//! (`spec/harness.md` "Oracle", WP-1254).
//!
//! A clang crash can depend on addresses (clang 19's unbounded
//! `AutoType::Profile` recursion, docs/VALIDATION.md "known oracle
//! defects"), so a second process usually gets through. Only the clang
//! oracle's compiles are retried, once: never kiln (the compiler under test
//! or a kiln oracle), never the oracle's binary, never a timeout, a memory
//! kill or a clean failure. A retry that succeeds leaves `advisory: oracle
//! crashed once (<signal>), retry ok` on the result; one that crashes again
//! is returned as is, so the caller reports the ERROR it always did.

use std::process::Command;
use std::time::Duration;

use super::{CompilerKind, Output, Problem, Test};

/// The `who` of the oracle's compiles ([`Test::exec`]); every other `who`
/// (the compiler under test, `the GCC oracle`, tools) runs once.
pub(super) const THE_ORACLE: &str = "the oracle";

impl Test<'_> {
    /// [`Self::exec_once`], and once more when `who` is the clang oracle and
    /// the first run [died of a signal](clang_crash). The success path runs
    /// one process.
    pub(super) fn exec(
        &mut self,
        who: &str,
        cmd: &Command,
        timeout: Duration,
    ) -> Result<Output, Problem> {
        let o = self.exec_once(who, cmd, timeout)?;
        if who != THE_ORACLE || self.cfg.oracle_kind != CompilerKind::Clang {
            return Ok(o);
        }
        let Some(first) = clang_crash(&o) else { return Ok(o) };
        self.log.push(format!("# the oracle crashed ({first}); retrying it once"));
        let retry = self.exec_once(who, cmd, timeout)?;
        if let Some(again) = clang_crash(&retry) {
            self.log.push(format!("# the oracle crashed again ({again}); not retried"));
        } else if retry.success() {
            self.add_advisory(format!("advisory: oracle crashed once ({first}), retry ok"));
        }
        Ok(retry)
    }

    /// Records an oracle advisory ([`TestResult::advisory`]); a second one
    /// in the same test is appended.
    pub(super) fn add_advisory(&mut self, note: String) {
        self.advisory = Some(match self.advisory.take() {
            Some(a) => format!("{a}; {note}"),
            None => note,
        });
    }
}

/// Did a clang compile die of a signal, other than by a memory kill? Then
/// the signal, as `signal N` (or the driver's own words when they name no
/// signal this table knows). Either the process itself was killed by a
/// signal, or the driver reports that a command it ran was: `<tool> command
/// failed due to signal`, with `unable to execute command: <strsignal
/// text>` naming it (an out-of-process `cc1`, as Debian's clang runs it),
/// or `... failed with exit code 128+N` (an in-process `cc1` the driver's
/// crash recovery caught). Never a timeout, a success, a clean failure, an
/// allocation failure ([`Output::out_of_memory`], or its markers on a
/// driver's exit 1) or SIGKILL (the kernel's OOM killer).
pub(super) fn clang_crash(o: &Output) -> Option<String> {
    if o.timed_out() || o.success() || o.out_of_memory() {
        return None;
    }
    let stderr = o.stderr_str();
    let oom = stderr.contains("LLVM ERROR: out of memory")
        || stderr.contains("std::bad_alloc")
        || stderr.contains("memory allocation of ") && stderr.contains(" failed")
        || stderr.contains("Cannot allocate memory");
    if oom {
        return None;
    }
    let signal = match o.signal() {
        Some(sig) => Signal::Number(sig),
        None => driver_signal(&stderr)?,
    };
    match signal {
        Signal::Number(SIGKILL) => None,
        Signal::Number(n) => Some(format!("signal {n}")),
        Signal::Described(text) => Some(text),
    }
}

const SIGKILL: i32 = 9;

enum Signal {
    Number(i32),
    /// A description no entry of [`SIGNAL_NAMES`] matches.
    Described(String),
}

/// glibc's `strsignal` texts, which LLVM's `ExecuteAndWait` reports.
const SIGNAL_NAMES: &[(&str, i32)] = &[
    ("Hangup", 1),
    ("Interrupt", 2),
    ("Quit", 3),
    ("Illegal instruction", 4),
    ("Trace/breakpoint trap", 5),
    ("Aborted", 6),
    ("Bus error", 7),
    ("Floating point exception", 8),
    ("Killed", SIGKILL),
    ("Segmentation fault", 11),
    ("Terminated", 15),
];

/// The signal a clang driver's stderr says one of its commands died of.
fn driver_signal(stderr: &str) -> Option<Signal> {
    let mut signalled = false;
    let mut described = None;
    for line in stderr.lines() {
        if line.contains(" command failed due to signal") {
            signalled = true;
        } else if let Some((_, code)) = line.split_once(" command failed with exit code ") {
            let code: i32 = code.split_whitespace().next()?.parse().ok()?;
            if (129..=192).contains(&code) {
                return Some(Signal::Number(code - 128));
            }
        } else if let Some((_, text)) = line.split_once("error: unable to execute command: ") {
            let text = text.trim_end();
            let text = text.strip_suffix(" (core dumped)").unwrap_or(text);
            described.get_or_insert(text.to_owned());
        }
    }
    if !signalled {
        return None;
    }
    Some(match described {
        Some(text) => SIGNAL_NAMES
            .iter()
            .find(|(name, _)| *name == text)
            .map_or(Signal::Described(text), |&(_, n)| Signal::Number(n)),
        None => Signal::Described("a command of the driver died of a signal".into()),
    })
}

#[cfg(all(test, unix))]
mod tests {
    use std::os::unix::process::ExitStatusExt;
    use std::process::ExitStatus;
    use std::time::Duration;

    use super::{Output, clang_crash};

    fn out(status: Option<i32>, stderr: &str) -> Output {
        Output {
            status: status.map(ExitStatus::from_raw),
            stdout: Vec::new(),
            stderr: stderr.as_bytes().to_vec(),
            elapsed: Duration::ZERO,
        }
    }

    const EXIT_1: Option<i32> = Some(1 << 8);
    const EXIT_254: Option<i32> = Some(254 << 8);

    /// The merge-group failure of WP-1254: Debian's clang 19 driver,
    /// its out-of-process `cc1` killed by SIGSEGV, exits 1.
    #[test]
    fn driver_reports_a_frontend_signal() {
        let stderr = "Stack dump:\n\
            clang++-19: error: unable to execute command: Segmentation fault (core dumped)\n\
            clang++-19: error: clang frontend command failed due to signal (use -v to see invocation)\n";
        assert_eq!(clang_crash(&out(EXIT_1, stderr)).as_deref(), Some("signal 11"));
        // `-fno-crash-diagnostics`: the driver's exit status is 254 (-2).
        let trap = "clang++-19: error: unable to execute command: Trace/breakpoint trap\n\
            clang++-19: error: clang frontend command failed due to signal (use -v to see invocation)\n";
        assert_eq!(clang_crash(&out(EXIT_254, trap)).as_deref(), Some("signal 5"));
        let odd = "clang: error: unable to execute command: Stack fault\n\
            clang: error: clang frontend command failed due to signal (use -v to see invocation)\n";
        assert_eq!(clang_crash(&out(EXIT_1, odd)).as_deref(), Some("Stack fault"));
        // An in-process `cc1` caught by the driver's crash recovery.
        let inproc = "clang: error: clang frontend command failed with exit code 139 (use -v to see invocation)\n";
        assert_eq!(clang_crash(&out(EXIT_1, inproc)).as_deref(), Some("signal 11"));
        let linker = "clang: error: unable to execute command: Bus error\n\
            clang: error: linker command failed due to signal (use -v to see invocation)\n";
        assert_eq!(clang_crash(&out(EXIT_1, linker)).as_deref(), Some("signal 7"));
    }

    #[test]
    fn the_process_itself_killed_by_a_signal() {
        assert_eq!(clang_crash(&out(Some(11), "")).as_deref(), Some("signal 11"));
        assert_eq!(clang_crash(&out(Some(6), "Aborted\n")).as_deref(), Some("signal 6"));
    }

    /// Timeouts, memory kills, SIGKILL and clean failures are not retried.
    #[test]
    fn what_is_not_a_crash() {
        assert_eq!(clang_crash(&out(None, "")), None, "a timeout");
        assert_eq!(clang_crash(&out(Some(0), "")), None, "a success");
        let reject = "t.cpp:1:1: error: unknown type name 'nope'\n1 error generated.\n";
        assert_eq!(clang_crash(&out(EXIT_1, reject)), None, "a rejection");
        let ice_exit_1 = "t.cpp: error: clang frontend command failed with exit code 1\n";
        assert_eq!(clang_crash(&out(EXIT_1, ice_exit_1)), None, "a plain failure code");
        assert_eq!(clang_crash(&out(Some(9), "")), None, "SIGKILL");
        let killed = "clang++-19: error: unable to execute command: Killed\n\
            clang++-19: error: clang frontend command failed due to signal (use -v to see invocation)\n";
        assert_eq!(clang_crash(&out(EXIT_1, killed)), None, "a SIGKILLed cc1");
        let oom = "LLVM ERROR: out of memory\nAllocation failed\n\
            clang++-19: error: unable to execute command: Aborted (core dumped)\n\
            clang++-19: error: clang frontend command failed due to signal (use -v to see invocation)\n";
        assert_eq!(clang_crash(&out(EXIT_1, oom)), None, "out of memory, exit 1");
        assert_eq!(clang_crash(&out(Some(6), oom)), None, "out of memory, aborted");
        let exec = "clang: error: unable to execute command: Executable \"ld\" doesn't exist!\n";
        assert_eq!(clang_crash(&out(EXIT_1, exec)), None, "no signal named");
    }
}
