//! Running a child process with captured output and a wall-clock timeout,
//! using only `std` (reader threads + `try_wait` polling), and running one for
//! measurement (`run_measured`: precise wall clock plus peak RSS); capping a
//! child's address space ([`MemLimit`]) and recognizing that it ran out
//! ([`Output::out_of_memory`]).

use std::fmt;
use std::io::{self, Read, Write};
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

    /// Did the process fail because an allocation failed (the signature a
    /// [`MemLimit`] leaves)? See [`out_of_memory`].
    pub fn out_of_memory(&self) -> bool {
        out_of_memory(self.status, &self.stderr_str())
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

/// The default address-space cap of every compiler, oracle and test-binary
/// invocation of `xtask test` and `xtask bench` (`spec/harness.md`
/// "Resource limits"), in MiB.
pub const DEFAULT_MEM_LIMIT_MIB: u64 = 8192;

/// An address-space cap (`RLIMIT_AS`) for a child, in MiB.
///
/// `std::process::Command` has no rlimit API, and `#![forbid(unsafe_code)]`
/// rules out `pre_exec` and FFI, so [`MemLimit::wrap`] runs the child through
/// POSIX `sh`: `sh -c 'ulimit -v "$1" || exit 125; shift; exec "$@"'
/// kiln-mem-limit <KiB> <program> <args...>`. `exec` replaces the shell, so
/// the capped process keeps the pid the caller waits on (timeouts, process
/// groups and GNU time's RSS are unaffected) and the limit is inherited by
/// its children.
///
/// A process over the cap sees its allocations fail: Rust aborts with
/// `memory allocation of N bytes failed`, C++ throws `std::bad_alloc`, the
/// dynamic loader cannot map a library. [`out_of_memory`] recognizes those.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct MemLimit {
    pub mib: u64,
}

impl MemLimit {
    /// The exit status of the wrapper when `ulimit` itself fails (the hard
    /// limit is lower than the requested cap).
    pub const ULIMIT_FAILED: i32 = 125;

    /// `--mem-limit <MiB>`: `0` disables the cap.
    pub fn from_mib(mib: u64) -> Option<Self> {
        (mib > 0).then_some(Self { mib })
    }

    /// The cap raised by `extra_mib` (saturating).
    #[must_use]
    pub fn plus(self, extra_mib: u64) -> Self {
        Self { mib: self.mib.saturating_add(extra_mib) }
    }

    /// `cmd` run under this cap: same program, arguments, working directory
    /// and explicit environment. Off Unix the command is returned as is.
    pub fn wrap(self, cmd: &Command) -> Command {
        if !cfg!(unix) {
            return clone_command(cmd, Command::new(cmd.get_program()));
        }
        let mut c = Command::new("sh");
        c.arg("-c")
            .arg(format!(
                "ulimit -v \"$1\" || exit {}; shift; exec \"$@\"",
                Self::ULIMIT_FAILED
            ))
            .arg("kiln-mem-limit")
            .arg(self.mib.saturating_mul(1024).to_string())
            .arg(cmd.get_program());
        clone_command(cmd, c)
    }
}

/// Copies `from`'s arguments, working directory and explicit environment
/// onto `to`.
fn clone_command(from: &Command, mut to: Command) -> Command {
    to.args(from.get_args());
    if let Some(d) = from.get_current_dir() {
        to.current_dir(d);
    }
    for (k, v) in from.get_envs() {
        match v {
            Some(v) => to.env(k, v),
            None => to.env_remove(k),
        };
    }
    to
}

/// `cmd` under `limit`, or a copy of it when there is none.
pub fn limited(cmd: &Command, limit: Option<MemLimit>) -> Command {
    match limit {
        Some(l) => l.wrap(cmd),
        None => clone_command(cmd, Command::new(cmd.get_program())),
    }
}

/// The address space kiln reserves up front for its compiling thread's
/// stack, in MiB: the last valid `-fkiln-stack-size=N[K|M|G]` in `args`,
/// else 1 GiB (`kiln_sema::stack_guard`; spec/sema-templates.md Q4). The
/// harness adds it to the cap of kiln invocations, so a budget measures
/// what the compile allocates rather than the reservation.
pub fn kiln_stack_reservation_mib<S: AsRef<std::ffi::OsStr>>(args: &[S]) -> u64 {
    const DEFAULT_MIB: u64 = 1024;
    const MIN_MIB: u64 = 4;
    let parse = |v: &str| -> Option<u64> {
        let (digits, shift) = match v.as_bytes().last()? {
            b'k' | b'K' => (&v[..v.len() - 1], 10),
            b'm' | b'M' => (&v[..v.len() - 1], 20),
            b'g' | b'G' => (&v[..v.len() - 1], 30),
            _ => (v, 0),
        };
        let n: u64 = digits.parse().ok().filter(|&n| n > 0)?;
        let bytes = n.checked_mul(1u64 << shift)?;
        Some(bytes.div_ceil(1 << 20).max(MIN_MIB))
    };
    args.iter()
        .rev()
        .filter_map(|a| a.as_ref().to_str()?.strip_prefix("-fkiln-stack-size="))
        .find_map(parse)
        .unwrap_or(DEFAULT_MIB)
}

/// Did a process that ended with `status` and wrote `stderr` fail because an
/// allocation failed? Only a process that did not succeed and was not timed
/// out qualifies, and only on an allocation-failure signature that ordinary
/// diagnostics cannot produce:
/// - killed by a signal (an abort; or exit status 128 + the signal, as a
///   shell or GNU time reports it) after Rust's `memory allocation of N
///   bytes failed`, an uncaught `std::bad_alloc`, or `LLVM ERROR: out of
///   memory`;
/// - any failure whose stderr has a line starting with Python's
///   `MemoryError`, the dynamic loader's `error while loading shared
///   libraries: ... failed to map segment`, or the shell's `Cannot allocate
///   memory` for an `exec` that could not map the program.
pub fn out_of_memory(status: Option<ExitStatus>, stderr: &str) -> bool {
    let Some(status) = status else { return false };
    if status.success() {
        return false;
    }
    let signalled = status.code().is_none_or(|c| c >= 128);
    let abort_marker = stderr.contains("memory allocation of ") && stderr.contains(" failed")
        || stderr.contains("std::bad_alloc")
        || stderr.contains("LLVM ERROR: out of memory");
    if signalled && abort_marker {
        return true;
    }
    stderr.lines().any(|l| {
        l.starts_with("MemoryError")
            || l.contains("error while loading shared libraries:")
                && l.contains("failed to map segment")
            || l.starts_with("kiln-mem-limit: ") && l.contains("Cannot allocate memory")
    })
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
    run_with_input(cmd, None, timeout)
}

/// Like [`run`], but feeds `input` to the child's stdin (then closes it) when
/// given. A child that exits without reading all of it is not an error.
pub fn run_with_input(
    cmd: &mut Command,
    input: Option<&[u8]>,
    timeout: Duration,
) -> io::Result<Output> {
    let start = Instant::now();
    let stdin = if input.is_some() { Stdio::piped() } else { Stdio::null() };
    let mut child = cmd.stdin(stdin).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()?;
    let writer = input.map(|bytes| {
        let mut pipe = child.stdin.take().expect("stdin is piped");
        let bytes = bytes.to_vec();
        thread::spawn(move || {
            // EPIPE (the child stopped reading) just ends the input early;
            // dropping `pipe` closes the child's stdin.
            let _ = pipe.write_all(&bytes);
        })
    });
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
    if let Some(w) = writer {
        let _ = w.join();
    }
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
    limit: Option<MemLimit>,
) -> io::Result<Measured> {
    let probed = match probe {
        RssProbe::GnuTime(time) => {
            let mut c = Command::new(time);
            c.args(["-f", "%M", "-o"]).arg(rss_file).arg("--").arg(inner.get_program());
            c
        }
        RssProbe::Unavailable => Command::new(inner.get_program()),
    };
    // The cap goes outside GNU time: `ulimit` then `exec`s time, which
    // forks the compiler with the limit inherited (time's own few hundred
    // KiB count against the cap; its RSS report is the compiler's alone).
    let mut cmd = limited(&clone_command(inner, probed), limit);
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
    fn feeds_stdin() {
        let o = run_with_input(
            Command::new("sh").args(["-c", "read a; read b; echo \"$b $a\"; cat"]),
            Some(b"one\ntwo\nrest\n"),
            Duration::from_secs(10),
        )
        .unwrap();
        assert_eq!(o.stdout_str(), "two one\nrest\n");
        // A child that ignores a large input still finishes normally.
        let big = vec![b'x'; 1 << 20];
        let o =
            run_with_input(&mut Command::new("true"), Some(&big), Duration::from_secs(10)).unwrap();
        assert!(o.success());
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
        let m = run_measured(&c, &probe, &dir.join("rss"), Duration::from_secs(30), None).unwrap();
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
        let m = run_measured(&c, &probe, &dir.join("rss"), Duration::from_millis(200), None)
                .unwrap();
        assert!(m.status.is_none());
        assert!(m.wall < Duration::from_secs(10), "{m:?}");
        assert_eq!(m.max_rss_kb, None);
        assert!(m.as_output().describe_end().starts_with("timed out"));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// The wrapper keeps the program's arguments (quoting included), working
    /// directory and environment, and applies the cap in KiB.
    #[cfg(unix)]
    #[test]
    fn mem_limit_wraps_transparently() {
        let dir = scratch("wrap");
        let mut c = Command::new("sh");
        c.args(["-c", "printf '%s|' \"$0\" \"$1\" \"$V\" \"$(pwd)\"; ulimit -v", "a b", "it's"])
            .current_dir(&dir)
            .env("V", "v=1");
        let w = MemLimit::from_mib(300).unwrap().wrap(&c);
        assert_eq!(w.get_program(), "sh");
        let o = run(&mut limited(&c, MemLimit::from_mib(300)), Duration::from_secs(10)).unwrap();
        let canon = std::fs::canonicalize(&dir).unwrap();
        assert_eq!(o.stdout_str(), format!("a b|it's|v=1|{}|307200\n", canon.display()));
        // No limit: the command as is.
        assert_eq!(MemLimit::from_mib(0), None);
        let o = run(&mut limited(&c, None), Duration::from_secs(10)).unwrap();
        assert!(o.stdout_str().ends_with("|unlimited\n"), "{}", o.stdout_str());
        assert_eq!(MemLimit { mib: 10 }.plus(1024), MemLimit { mib: 1034 });
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// A process that allocates past the cap fails, and is recognized as
    /// out of memory; under the default cap the same process succeeds.
    #[cfg(unix)]
    #[test]
    fn mem_limit_stops_a_hog() {
        if run(Command::new("python3").arg("--version"), Duration::from_secs(10))
            .map_or(true, |o| !o.success())
        {
            eprintln!("python3 not installed; skipping the memory-limit test");
            return;
        }
        let mut hog = Command::new("python3");
        hog.args(["-c", "b = bytearray(1 << 30); print(len(b))"]);
        let o = run(&mut limited(&hog, MemLimit::from_mib(256)), Duration::from_secs(30)).unwrap();
        assert!(!o.success() && o.out_of_memory(), "{o:?}");
        let o = run(&mut limited(&hog, MemLimit::from_mib(DEFAULT_MEM_LIMIT_MIB)), Duration::from_secs(30))
            .unwrap();
        assert!(o.success() && !o.out_of_memory(), "{o:?}");
        // The cap applies under the RSS probe, too.
        let dir = scratch("hog");
        let probe = RssProbe::detect(&dir);
        let m = run_measured(&hog, &probe, &dir.join("rss"), Duration::from_secs(30), MemLimit::from_mib(256))
            .unwrap();
        assert!(m.as_output().out_of_memory(), "{m:?}");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn out_of_memory_signatures() {
        use std::os::unix::process::ExitStatusExt;
        let exit = |c: i32| Some(ExitStatus::from_raw(c << 8));
        let signal = |s: i32| Some(ExitStatus::from_raw(s));
        let rust = "memory allocation of 8388608 bytes failed\n";
        let cxx = "terminate called after throwing an instance of 'std::bad_alloc'\n  what():  std::bad_alloc\n";
        for text in [rust, cxx, "LLVM ERROR: out of memory\n"] {
            assert!(out_of_memory(signal(6), text), "{text}");
            assert!(out_of_memory(exit(134), text), "{text}");
            // A diagnostic quoting the words exits normally: not a kill.
            assert!(!out_of_memory(exit(1), text), "{text}");
            assert!(!out_of_memory(exit(0), text), "{text}");
        }
        let py = "Traceback (most recent call last):\n  File \"<string>\", line 1\nMemoryError\n";
        assert!(out_of_memory(exit(1), py));
        let ld = "clang++-19: error while loading shared libraries: libLLVM.so.19.1: failed to map segment from shared object\n";
        assert!(out_of_memory(exit(127), ld));
        assert!(out_of_memory(exit(2), "kiln-mem-limit: 1: exec: /bin/x: Cannot allocate memory\n"));
        // Other failures are not.
        assert!(!out_of_memory(signal(11), "Segmentation fault\n"));
        assert!(!out_of_memory(exit(1), "t.cpp:3:1: error: MemoryError is not a type\n"));
        assert!(!out_of_memory(None, rust), "a timeout is not a memory kill");
        assert!(!out_of_memory(exit(0), py));
    }

    #[test]
    fn kiln_stack_reservation() {
        assert_eq!(kiln_stack_reservation_mib::<&str>(&[]), 1024);
        assert_eq!(kiln_stack_reservation_mib(&["-O2", "-fkiln-stack-size=8M"]), 8);
        assert_eq!(kiln_stack_reservation_mib(&["-fkiln-stack-size=2G"]), 2048);
        assert_eq!(kiln_stack_reservation_mib(&["-fkiln-stack-size=100k"]), 4, "raised to the minimum");
        assert_eq!(kiln_stack_reservation_mib(&["-fkiln-stack-size=5242881"]), 6, "rounded up");
        // The last valid one wins, as in kiln; an invalid one is skipped.
        assert_eq!(kiln_stack_reservation_mib(&["-fkiln-stack-size=64M", "-fkiln-stack-size=x"]), 64);
        assert_eq!(kiln_stack_reservation_mib(&["-fkiln-stack-size=64M", "-fkiln-stack-size=32M"]), 32);
        assert_eq!(kiln_stack_reservation_mib(&["-fkiln-stack-size=0"]), 1024);
    }

    #[test]
    fn shown_quotes() {
        let mut c = Command::new("cc");
        c.args(["-DX=a b", "plain", "it's"]);
        assert_eq!(Shown(&c).to_string(), r"cc '-DX=a b' plain 'it'\''s'");
    }
}
