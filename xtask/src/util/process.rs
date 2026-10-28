//! Running a child process with captured output and a wall-clock timeout,
//! using only `std` (reader threads + `try_wait` polling), and running one for
//! measurement (`run_measured`: precise wall clock plus peak RSS).

use std::fmt;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

/// The result of a finished (or killed) child.
#[derive(Debug)]
pub struct Output {
    /// `None` if the process was killed for exceeding the timeout.
    pub status: Option<ExitStatus>,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub elapsed: Duration,
}

impl Output {
    pub fn timed_out(&self) -> bool {
        self.status.is_none()
    }

    pub fn success(&self) -> bool {
        self.status.is_some_and(|s| s.success())
    }

    pub fn stdout_str(&self) -> String {
        String::from_utf8_lossy(&self.stdout).into_owned()
    }

    pub fn stderr_str(&self) -> String {
        String::from_utf8_lossy(&self.stderr).into_owned()
    }

    /// Exit code, or `None` if killed (by timeout or a signal).
    pub fn code(&self) -> Option<i32> {
        self.status.and_then(|s| s.code())
    }

    /// Describes how the process ended: `exit code 3`, `signal 11`, `timed out`.
    pub fn describe_end(&self) -> String {
        match self.status {
            None => format!("timed out after {:.1}s", self.elapsed.as_secs_f64()),
            Some(s) => match s.code() {
                Some(c) => format!("exit code {c}"),
                None => signal_of(s).map_or_else(
                    || "terminated abnormally".to_owned(),
                    |sig| format!("killed by signal {sig}"),
                ),
            },
        }
    }
}

#[cfg(unix)]
fn signal_of(s: ExitStatus) -> Option<i32> {
    std::os::unix::process::ExitStatusExt::signal(&s)
}

#[cfg(not(unix))]
fn signal_of(_: ExitStatus) -> Option<i32> {
    None
}

/// Renders a command for logs: program and arguments, space-separated, with
/// arguments containing whitespace or quotes single-quoted.
pub struct Shown<'a>(pub &'a Command);

impl fmt::Display for Shown<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let quote = |s: &str| {
            if s.is_empty() || s.contains(|c: char| c.is_whitespace() || "'\"\\$".contains(c)) {
                format!("'{}'", s.replace('\'', r"'\''"))
            } else {
                s.to_owned()
            }
        };
        write!(f, "{}", quote(&self.0.get_program().to_string_lossy()))?;
        for a in self.0.get_args() {
            write!(f, " {}", quote(&a.to_string_lossy()))?;
        }
        Ok(())
    }
}

/// Runs `cmd` with stdin closed, capturing stdout and stderr, killing it
/// after `timeout`.
pub fn run(cmd: &mut Command, timeout: Duration) -> io::Result<Output> {
    let start = Instant::now();
    let mut child =
        cmd.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()?;
    let out_reader = spawn_reader(&mut child, true);
    let err_reader = spawn_reader(&mut child, false);
    let mut delay = Duration::from_millis(1);
    let status = loop {
        if let Some(s) = child.try_wait()? {
            break Some(s);
        }
        if start.elapsed() >= timeout {
            // The child may exit between try_wait and kill; both are fine.
            let _ = child.kill();
            let _ = child.wait();
            break None;
        }
        thread::sleep(delay);
        delay = (delay * 2).min(Duration::from_millis(25));
    };
    let stdout = out_reader.join().unwrap_or_default();
    let stderr = err_reader.join().unwrap_or_default();
    Ok(Output { status, stdout, stderr, elapsed: start.elapsed() })
}

fn spawn_reader(child: &mut Child, stdout: bool) -> thread::JoinHandle<Vec<u8>> {
    let mut pipe: Box<dyn Read + Send> = if stdout {
        Box::new(child.stdout.take().expect("stdout is piped"))
    } else {
        Box::new(child.stderr.take().expect("stderr is piped"))
    };
    thread::spawn(move || {
        let mut buf = Vec::new();
        // A read error (e.g. the child was killed) just truncates the capture.
        let _ = pipe.read_to_end(&mut buf);
        buf
    })
}

/// How the peak resident set size of a measured child is obtained.
///
/// `std` cannot call `wait4`/`getrusage`, and `#![forbid(unsafe_code)]` rules
/// out FFI, so the portable safe route is GNU time's `%M` (the child's
/// `ru_maxrss` in KiB, as reported by `wait4`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RssProbe {
    /// Wrap the command as `<time> -f %M -o <file> -- <command...>`.
    GnuTime(PathBuf),
    /// Wall clock only.
    Unavailable,
}

impl RssProbe {
    /// The well-known location of GNU time.
    pub const GNU_TIME: &'static str = "/usr/bin/time";

    /// Uses `/usr/bin/time` if it accepts `-f %M -o <file>` and writes a
    /// number; otherwise `Unavailable`. `scratch` is a writable directory.
    pub fn detect(scratch: &Path) -> Self {
        Self::detect_at(Path::new(Self::GNU_TIME), scratch)
    }

    pub fn detect_at(time: &Path, scratch: &Path) -> Self {
        let file = scratch.join("rss-probe.txt");
        let ok = run(
            Command::new(time).args(["-f", "%M", "-o"]).arg(&file).args(["--", "true"]),
            Duration::from_secs(10),
        )
        .is_ok_and(|o| o.success())
            && read_rss_file(&file).is_some();
        let _ = std::fs::remove_file(&file);
        if ok { Self::GnuTime(time.to_path_buf()) } else { Self::Unavailable }
    }

    /// The `rss_source` value recorded in result files.
    pub fn source_id(&self) -> &'static str {
        match self {
            Self::GnuTime(_) => "gnu-time",
            Self::Unavailable => "none",
        }
    }

    pub fn describe(&self) -> String {
        match self {
            Self::GnuTime(p) => format!("max RSS via {} -f %M", p.display()),
            Self::Unavailable => {
                format!("max RSS unavailable ({} not usable); wall clock only", Self::GNU_TIME)
            }
        }
    }
}

/// One measured run of a child.
#[derive(Debug)]
pub struct Measured {
    /// `None` if the process was killed for exceeding the timeout.
    pub status: Option<ExitStatus>,
    pub stderr: Vec<u8>,
    /// From just before spawn to the moment `wait` returned.
    pub wall: Duration,
    /// Peak RSS of the child in KiB, when the probe could measure it.
    pub max_rss_kb: Option<u64>,
}

impl Measured {
    pub fn as_output(&self) -> Output {
        Output {
            status: self.status,
            stdout: Vec::new(),
            stderr: self.stderr.clone(),
            elapsed: self.wall,
        }
    }
}

/// Runs `inner` (its program, arguments and working directory) for
/// measurement: stdin and stdout are null, stderr is captured, and the wall
/// clock is taken by a thread blocked in `wait`, so it has no polling
/// quantum. The child runs in its own process group so that a timeout kills
/// the wrapper and the compiler together. `rss_file` is a scratch path for
/// the probe's report.
pub fn run_measured(
    inner: &Command,
    probe: &RssProbe,
    rss_file: &Path,
    timeout: Duration,
) -> io::Result<Measured> {
    let mut cmd = match probe {
        RssProbe::GnuTime(time) => {
            let mut c = Command::new(time);
            c.args(["-f", "%M", "-o"]).arg(rss_file).arg("--").arg(inner.get_program());
            c
        }
        RssProbe::Unavailable => Command::new(inner.get_program()),
    };
    cmd.args(inner.get_args());
    if let Some(d) = inner.get_current_dir() {
        cmd.current_dir(d);
    }
    #[cfg(unix)]
    std::os::unix::process::CommandExt::process_group(&mut cmd, 0);
    cmd.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::piped());
    let _ = std::fs::remove_file(rss_file);

    let start = Instant::now();
    let mut child = cmd.spawn()?;
    let pid = child.id();
    let err_reader = spawn_reader(&mut child, false);
    let (tx, rx) = mpsc::channel();
    let waiter = thread::spawn(move || {
        let r = child.wait();
        let _ = tx.send((r, Instant::now()));
    });
    let (status, end) = if let Ok((r, end)) = rx.recv_timeout(timeout) {
        (Some(r?), end)
    } else {
        kill_group(pid);
        let Ok((r, end)) = rx.recv_timeout(Duration::from_secs(30)) else {
            return Err(io::Error::other(format!(
                "process {pid} survived a kill after timing out"
            )));
        };
        r?;
        (None, end)
    };
    let _ = waiter.join();
    let stderr = err_reader.join().unwrap_or_default();
    let max_rss_kb = match (probe, status) {
        (RssProbe::GnuTime(_), Some(_)) => read_rss_file(rss_file),
        _ => None,
    };
    let _ = std::fs::remove_file(rss_file);
    Ok(Measured { status, stderr, wall: end.duration_since(start), max_rss_kb })
}

/// GNU time writes `Command exited with non-zero status N` (or `Command
/// terminated by signal N`) before the formatted line; the number is last.
fn read_rss_file(path: &Path) -> Option<u64> {
    let text = std::fs::read_to_string(path).ok()?;
    text.lines().rev().find(|l| !l.trim().is_empty())?.trim().parse().ok()
}

#[cfg(unix)]
fn kill_group(pid: u32) {
    // The child is the leader of its own process group (pgid == pid); the
    // pid cannot be reused before our waiter thread reaps it.
    let _ = Command::new("kill")
        .args(["-KILL", "--", &format!("-{pid}")])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

#[cfg(not(unix))]
fn kill_group(_pid: u32) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn captures_and_reports_exit() {
        let o = run(
            Command::new("sh").args(["-c", "echo out; echo err >&2; exit 3"]),
            Duration::from_secs(10),
        )
        .unwrap();
        assert_eq!(o.stdout_str(), "out\n");
        assert_eq!(o.stderr_str(), "err\n");
        assert_eq!(o.code(), Some(3));
        assert_eq!(o.describe_end(), "exit code 3");
        assert!(!o.success() && !o.timed_out());
    }

    #[test]
    fn times_out() {
        let o = run(Command::new("sleep").arg("5"), Duration::from_millis(100)).unwrap();
        assert!(o.timed_out());
        assert!(o.elapsed < Duration::from_secs(4));
        assert!(o.describe_end().starts_with("timed out"));
    }

    fn scratch(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("xtask-process-{tag}-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn measured_run_reports_wall_and_rss() {
        let dir = scratch("measure");
        let probe = RssProbe::detect(&dir);
        let mut c = Command::new("sh");
        c.args(["-c", "echo diag >&2; exit 2"]);
        let m = run_measured(&c, &probe, &dir.join("rss"), Duration::from_secs(30)).unwrap();
        assert_eq!(m.status.and_then(|s| s.code()), Some(2));
        assert_eq!(String::from_utf8_lossy(&m.stderr), "diag\n");
        assert!(m.wall > Duration::ZERO && m.wall < Duration::from_secs(20));
        match probe {
            // Even a failing child's RSS is read (past GNU time's status line).
            RssProbe::GnuTime(_) => assert!(m.max_rss_kb.is_some_and(|k| k > 0), "{m:?}"),
            RssProbe::Unavailable => assert_eq!(m.max_rss_kb, None),
        }
        assert_eq!(
            RssProbe::detect_at(Path::new("/nonexistent/time"), &dir),
            RssProbe::Unavailable
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn measured_run_times_out_and_kills_the_group() {
        let dir = scratch("timeout");
        let probe = RssProbe::detect(&dir);
        let mut c = Command::new("sleep");
        c.arg("30");
        let m = run_measured(&c, &probe, &dir.join("rss"), Duration::from_millis(200)).unwrap();
        assert!(m.status.is_none());
        assert!(m.wall < Duration::from_secs(10), "{m:?}");
        assert_eq!(m.max_rss_kb, None);
        assert!(m.as_output().describe_end().starts_with("timed out"));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn shown_quotes() {
        let mut c = Command::new("cc");
        c.args(["-DX=a b", "plain", "it's"]);
        assert_eq!(Shown(&c).to_string(), r"cc '-DX=a b' plain 'it'\''s'");
    }
}
