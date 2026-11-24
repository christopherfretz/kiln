//! Running a child process with captured output and a wall-clock timeout,
//! using only `std` (reader threads + `try_wait` polling), and running one for
//! measurement (`run_measured`: precise wall clock plus peak RSS); capping a
//! child's address space ([`MemLimit`]) and recognizing that it ran out
//! ([`Output::out_of_memory`]).
//!
//! Every child either function starts is registered in a process-wide table
//! of live children (each the leader of its own process group) until it is
//! reaped, so that [`kill_live_children`], which the SIGINT/SIGTERM thread of
//! [`crate::jobs`] calls, can kill and reap all of them before xtask exits;
//! after it no new child starts (spawning fails with `Interrupted`).
//! [`run_step`] runs a cargo step the same way, but without a timeout or a
//! jobserver token, and is stopped with `SIGTERM` before `SIGKILL`.

use std::fmt;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::{Condvar, Mutex, MutexGuard, PoisonError, mpsc};
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

    /// The signal that killed the process, if one did (not a timeout).
    pub fn signal(&self) -> Option<i32> {
        self.status.and_then(signal_of)
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
/// POSIX `sh`: `sh -c 'ulimit -c 0 2>/dev/null; ulimit -v "$1" || exit 125;
/// shift; exec "$@"' kiln-mem-limit <KiB> <program> <args...>`. `exec`
/// replaces the shell, so the capped process keeps the pid the caller waits
/// on (timeouts, process groups and GNU time's RSS are unaffected) and the
/// limits are inherited by its children. Core dumps go off in the same
/// shell (WP-2762): a capped compiler that crashes writes no `core`.
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
                "ulimit -c 0 2>/dev/null; ulimit -v \"$1\" || exit {}; shift; exec \"$@\"",
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

/// `cmd` with core dumps off (`ulimit -c 0` in a `sh` wrapper that then
/// `exec`s the program, so a signal still ends the process as it would
/// have; off Unix a copy). A test binary that crashes is reported by its
/// signal at once: a core dump, or a core-dump helper such as apport or
/// systemd-coredump on a loaded host, took long enough to hit the
/// harness's run timeout (WP-1888). The program, arguments, working
/// directory and explicit environment are kept. When core dumps are
/// already off in this process ([`core_dumps_off_here`]: xtask turned them
/// off for itself at startup, [`exec_without_core_dumps`]) the child
/// inherits that, and `cmd` is returned as a copy with no wrapper.
pub fn without_core_dumps(cmd: &Command) -> Command {
    if !cfg!(unix) || core_dumps_off_here() {
        return clone_command(cmd, Command::new(cmd.get_program()));
    }
    no_core_wrapper(cmd)
}

/// `sh -c 'ulimit -c 0 2>/dev/null; exec "$@"' kiln-no-core <cmd...>`.
fn no_core_wrapper(cmd: &Command) -> Command {
    let mut c = Command::new("sh");
    c.arg("-c")
        .arg("ulimit -c 0 2>/dev/null; exec \"$@\"")
        .arg("kiln-no-core")
        .arg(cmd.get_program());
    clone_command(cmd, c)
}

/// Set on the xtask that [`exec_without_core_dumps`] re-executes, so that
/// it is tried once per run whatever the outcome.
pub const NO_CORE_ENV: &str = "KILN_XTASK_NO_CORE";

/// Turns core dumps off for this process and every process it will start
/// (WP-2762; `spec/harness.md` "Resource limits"). Called first thing in
/// xtask's `main`: unless they are off already ([`core_dumps_off_here`])
/// or [`NO_CORE_ENV`] is set, the process replaces itself (same pid, same
/// descriptors, process group and signal dispositions) with `sh -c 'ulimit
/// -c 0 2>/dev/null; exec "$@"' kiln-no-core <this executable> <args...>`,
/// which `exec`s xtask again with `RLIMIT_CORE` 0. Children inherit the
/// limit, so a crashing compiler, oracle, test binary, cargo test or
/// extents clang leaves no `core` in its working directory, whatever
/// route started it. `setrlimit` itself needs FFI, which
/// `#![forbid(unsafe_code)]` rules out; `CommandExt::exec` is safe. Returns
/// only when nothing was executed (not Unix, already off, tried once, or
/// the `exec` failed: the run then goes on with the host's setting).
pub fn exec_without_core_dumps() {
    #[cfg(unix)]
    {
        if std::env::var_os(NO_CORE_ENV).is_some() || core_dumps_off_here() {
            return;
        }
        let Ok(exe) = std::env::current_exe() else { return };
        let mut again = Command::new(exe);
        again.args(std::env::args_os().skip(1));
        let _ = std::os::unix::process::CommandExt::exec(&mut core_off_reexec(&again));
    }
}

/// What [`exec_without_core_dumps`] executes in place of xtask: `again`
/// (xtask's executable and arguments) under `ulimit -c 0`, with
/// [`NO_CORE_ENV`] set.
fn core_off_reexec(again: &Command) -> Command {
    let mut c = no_core_wrapper(again);
    c.env(NO_CORE_ENV, "1");
    c
}

/// Is this process's soft `RLIMIT_CORE` 0? Read once from
/// `/proc/self/limits` (`false` where it cannot be read, as off Linux).
pub fn core_dumps_off_here() -> bool {
    static OFF: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *OFF.get_or_init(|| {
        std::fs::read_to_string("/proc/self/limits").is_ok_and(|l| core_limit_is_zero(&l))
    })
}

/// The `Max core file size` row of a `/proc/<pid>/limits` text has a soft
/// limit of 0.
fn core_limit_is_zero(limits: &str) -> bool {
    limits
        .lines()
        .find_map(|l| l.strip_prefix("Max core file size"))
        .and_then(|rest| rest.split_whitespace().next())
        .is_some_and(|soft| soft == "0")
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
    // One jobserver slot for the child's lifetime (released on every return
    // path, kill or panic by `Drop`); the timeout starts once it is held.
    let _slot = crate::jobs::global().acquire();
    let start = Instant::now();
    let stdin = if input.is_some() { Stdio::piped() } else { Stdio::null() };
    // Its own process group, so that a timeout kills the whole tree: the
    // clang driver forks `cc1`, which inherits our pipes, and killing only
    // the driver left the readers blocked on a running `cc1` forever.
    #[cfg(unix)]
    std::os::unix::process::CommandExt::process_group(cmd, 0);
    let spawning = LIVE.begin_spawn()?;
    let child = cmd.stdin(stdin).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn();
    // Deregistered when this returns, after the child is reaped.
    let _live = spawning.spawned(child.as_ref().map(Child::id).ok(), Stop::Kill);
    let mut child = child?;
    let pid = child.id();
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
            // The child may exit between try_wait and kill; both are fine
            // (the pid is not reaped, so not reused, until `wait`).
            kill_group(pid);
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

/// The stack of [`run_streaming`]'s reader thread (address space; only what
/// is used is resident).
const STREAM_READER_STACK: usize = 256 << 20;

/// Like [`run`], but the child's standard output goes to `read` as it
/// arrives (on a thread of its own) instead of being captured, so an
/// output of any size is read in the memory `read` needs; the returned
/// [`Output`] has an empty `stdout`. `read` should read to the end of
/// the stream: a child blocked on a full pipe is killed at `timeout`.
pub fn run_streaming<T: Send + 'static>(
    cmd: &mut Command,
    timeout: Duration,
    read: impl FnOnce(std::process::ChildStdout) -> T + Send + 'static,
) -> io::Result<(Output, Option<T>)> {
    let _slot = crate::jobs::global().acquire();
    let start = Instant::now();
    #[cfg(unix)]
    std::os::unix::process::CommandExt::process_group(cmd, 0);
    let spawning = LIVE.begin_spawn()?;
    let child = cmd.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn();
    let _live = spawning.spawned(child.as_ref().map(Child::id).ok(), Stop::Kill);
    let mut child = child?;
    let pid = child.id();
    let pipe = child.stdout.take().expect("stdout is piped");
    // A big stack: `read` may recurse once per level of a deep tree.
    let out_reader =
        thread::Builder::new().stack_size(STREAM_READER_STACK).spawn(move || read(pipe))?;
    let err_reader = spawn_reader(&mut child, false);
    let mut delay = Duration::from_millis(1);
    let status = loop {
        if let Some(s) = child.try_wait()? {
            break Some(s);
        }
        if start.elapsed() >= timeout {
            kill_group(pid);
            let _ = child.kill();
            let _ = child.wait();
            break None;
        }
        thread::sleep(delay);
        delay = (delay * 2).min(Duration::from_millis(25));
    };
    let value = out_reader.join().ok();
    let stderr = err_reader.join().unwrap_or_default();
    Ok((Output { status, stdout: Vec::new(), stderr, elapsed: start.elapsed() }, value))
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

/// Runs a cargo step (the `cargo fmt`/`clippy`/`test` steps of `cargo
/// xtask ci`, the `cargo build` of [`super::build_kiln`]) to completion:
/// no timeout and no jobserver token of xtask's (cargo is a jobserver
/// client itself, its rustc and test processes take their own). stdin is
/// closed and stdout inherited. With `tee` stderr is piped, copied to
/// ours line by line as it arrives and returned; without it, inherited
/// (the returned string is empty).
///
/// When the SIGINT/SIGTERM thread of [`crate::jobs`] runs, the step is the
/// leader of its own process group and in the live-children table from
/// spawn to reap, so [`kill_live_children`] stops cargo and everything it
/// started (rustc, the test binaries): `SIGTERM` to the group first,
/// `SIGKILL` to whatever is left after [`TERM_GRACE`]. Without that
/// thread it stays in xtask's group, where a terminal ^C reaches it
/// directly, as before.
///
/// A step that ended because xtask is being interrupted does not return:
/// the calling thread waits for the signal thread to finish returning the
/// jobserver tokens and end the process, instead of racing it to `exit`
/// ([`crate::jobs::exit_interrupted`], which ends the process itself should
/// the signal thread not do it in time).
pub fn run_step(cmd: &mut Command, tee: bool) -> io::Result<(ExitStatus, String)> {
    run_step_hiding(cmd, tee, None)
}

/// [`run_step`] with `tee`, except that stderr lines containing `hide` are
/// captured but not copied to ours (the PGO final build's per-function
/// warnings, which only their count is shown of).
pub fn run_step_hiding(
    cmd: &mut Command,
    tee: bool,
    hide: Option<&str>,
) -> io::Result<(ExitStatus, String)> {
    let table = crate::jobs::signals_caught().then_some(&LIVE);
    let r = run_step_in(table, cmd, tee, hide);
    if table.is_some_and(|t| t.lock().shutdown) {
        crate::jobs::exit_interrupted();
    }
    r
}

fn run_step_in(
    table: Option<&LiveChildren>,
    cmd: &mut Command,
    tee: bool,
    hide: Option<&str>,
) -> io::Result<(ExitStatus, String)> {
    cmd.stdin(Stdio::null());
    if tee {
        cmd.stderr(Stdio::piped());
    }
    let (mut child, live) = match table {
        Some(table) => {
            #[cfg(unix)]
            std::os::unix::process::CommandExt::process_group(cmd, 0);
            let spawning = table.begin_spawn()?;
            let child = cmd.spawn();
            let live = spawning.spawned(child.as_ref().map(Child::id).ok(), Stop::Term);
            (child?, Some(live))
        }
        None => (cmd.spawn()?, None),
    };
    let mut captured = Vec::new();
    if let Some(pipe) = child.stderr.take() {
        let mut reader = io::BufReader::new(pipe);
        let mut chunk = Vec::new();
        let mut prev_hidden = false;
        let mut out = io::stderr();
        loop {
            chunk.clear();
            match io::BufRead::read_until(&mut reader, b'\n', &mut chunk) {
                Ok(0) | Err(_) => break,
                Ok(_) => {
                    // A hidden line takes the blank line after it along.
                    let text = String::from_utf8_lossy(&chunk);
                    let hidden = hide.is_some_and(|h| text.contains(h))
                        || (prev_hidden && text.trim().is_empty());
                    prev_hidden = hidden;
                    if !hidden {
                        let _ = out.write_all(&chunk);
                    }
                    captured.extend_from_slice(&chunk);
                }
            }
        }
    }
    let status = child.wait();
    // Reaped: only now may the pid leave the table.
    drop(live);
    Ok((status?, String::from_utf8_lossy(&captured).into_owned()))
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

    // As in `run_with_input`: a jobserver slot for the child's lifetime,
    // taken before the wall clock starts.
    let _slot = crate::jobs::global().acquire();
    let spawning = LIVE.begin_spawn()?;
    let start = Instant::now();
    let child = cmd.spawn();
    let live = spawning.spawned(child.as_ref().map(Child::id).ok(), Stop::Kill);
    let mut child = child?;
    let pid = child.id();
    let err_reader = spawn_reader(&mut child, false);
    let (tx, rx) = mpsc::channel();
    let waiter = thread::spawn(move || {
        let r = child.wait();
        let end = Instant::now();
        // Reaped: leave the table before anyone else can reuse the pid.
        drop(live);
        let _ = tx.send((r, end));
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

/// The live children of this process: the pids (each its own process-group
/// leader) between spawn and reap, the spawns in flight, and whether
/// [`kill_live_children`] has run.
struct LiveChildren {
    state: Mutex<LiveState>,
    changed: Condvar,
}

#[derive(Default)]
struct LiveState {
    pids: Vec<(u32, Stop)>,
    spawning: usize,
    shutdown: bool,
}

static LIVE: LiveChildren = LiveChildren::new();

/// How [`kill_live_children`] stops a live child's process group.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Stop {
    /// `SIGKILL` at once: a compiler or test binary of `util::process`.
    Kill,
    /// `SIGTERM`, then `SIGKILL` after [`TERM_GRACE`]: a cargo step of
    /// [`run_step`], whose processes may still clean up (a test binary's
    /// temporary files, cargo's own `Drop`s).
    Term,
}

/// How long a [`Stop::Term`] group gets after `SIGTERM` before `SIGKILL`.
pub const TERM_GRACE: Duration = Duration::from_secs(3);

/// A spawn in flight: counted so that a shutdown waits for (and kills) a
/// child whose pid is not known yet. Always ended by [`Spawning::spawned`]
/// right after the `spawn` call.
#[must_use]
struct Spawning<'a>(&'a LiveChildren);

/// A registered child, removed from the table on drop (after it is reaped,
/// or on any early return or panic).
#[must_use]
struct LiveChild<'a> {
    table: &'a LiveChildren,
    pid: Option<u32>,
}

impl LiveChildren {
    const fn new() -> Self {
        Self {
            state: Mutex::new(LiveState { pids: Vec::new(), spawning: 0, shutdown: false }),
            changed: Condvar::new(),
        }
    }

    /// The state is a pid list and two counters updated under the lock, so
    /// a poisoned lock still holds a consistent value.
    fn lock(&self) -> MutexGuard<'_, LiveState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn begin_spawn(&self) -> io::Result<Spawning<'_>> {
        let mut st = self.lock();
        if st.shutdown {
            return Err(io::Error::new(io::ErrorKind::Interrupted, "xtask is shutting down"));
        }
        st.spawning += 1;
        Ok(Spawning(self))
    }

    /// Sets the shutdown flag, stops the process group of every live child
    /// ([`Stop`]: `SIGKILL`, or `SIGTERM` and `SIGKILL` after `term_grace`
    /// for one still live then), and waits (at most `grace`) until each is
    /// reaped and no spawn is in flight; a child spawned meanwhile is
    /// killed as it registers. Returns how many groups were stopped.
    fn shutdown(&self, grace: Duration, term_grace: Duration) -> usize {
        let start = Instant::now();
        let deadline = start + grace;
        let mut escalate_at = Some(start + term_grace.min(grace));
        let mut st = self.lock();
        st.shutdown = true;
        // Signalled under the lock: a pid leaves the table only once reaped,
        // so none of these can have been reused.
        let stopped = st.pids.len();
        for &(pid, stop) in &st.pids {
            signal_group(pid, if stop == Stop::Term { "TERM" } else { "KILL" });
        }
        while !(st.pids.is_empty() && st.spawning == 0) {
            let now = Instant::now();
            if escalate_at.is_some_and(|at| now >= at) {
                for &(pid, stop) in &st.pids {
                    if stop == Stop::Term {
                        signal_group(pid, "KILL");
                    }
                }
                escalate_at = None;
            }
            let left = escalate_at.unwrap_or(deadline).saturating_duration_since(now);
            if left.is_zero() {
                break;
            }
            st = self.changed.wait_timeout(st, left).unwrap_or_else(PoisonError::into_inner).0;
        }
        stopped
    }
}

impl<'a> Spawning<'a> {
    /// Ends the spawn: registers `pid` (the spawn's result) with how a
    /// shutdown stops it, killing it at once if a shutdown started
    /// meanwhile (it has had no time to do anything worth cleaning up).
    fn spawned(self, pid: Option<u32>, stop: Stop) -> LiveChild<'a> {
        let table = self.0;
        let mut st = table.lock();
        st.spawning -= 1;
        if let Some(pid) = pid {
            st.pids.push((pid, stop));
            if st.shutdown {
                kill_group(pid);
            }
        }
        drop(st);
        table.changed.notify_all();
        LiveChild { table, pid }
    }
}

impl Drop for LiveChild<'_> {
    fn drop(&mut self) {
        let Some(pid) = self.pid else { return };
        let mut st = self.table.lock();
        if let Some(i) = st.pids.iter().position(|&(p, _)| p == pid) {
            st.pids.swap_remove(i);
        }
        drop(st);
        self.table.changed.notify_all();
    }
}

/// Kills (`SIGKILL`, by process group) every child started by [`run`],
/// [`run_with_input`] or [`run_measured`] that is still running, stops
/// every [`run_step`] cargo step (`SIGTERM`, then `SIGKILL` after
/// `term_grace`), waits up to `grace` for their reaping, and makes every
/// later spawn fail. Called once, by the SIGINT/SIGTERM thread, before the
/// jobserver tokens are returned. Returns how many children were stopped.
pub fn kill_live_children(grace: Duration, term_grace: Duration) -> usize {
    LIVE.shutdown(grace, term_grace)
}

/// `SIGKILL` to every live child's group, without waiting for anything, and
/// every later spawn fails: a second signal during [`kill_live_children`],
/// or its time bound (the process ends right after). Skipped if the table
/// is locked (by a cleanup stuck while holding it).
pub fn kill_live_children_now() {
    let mut st = match LIVE.state.try_lock() {
        Ok(st) => st,
        Err(std::sync::TryLockError::Poisoned(p)) => p.into_inner(),
        Err(std::sync::TryLockError::WouldBlock) => return,
    };
    st.shutdown = true;
    for &(pid, _) in &st.pids {
        kill_group(pid);
    }
}

#[cfg(unix)]
fn kill_group(pid: u32) {
    signal_group(pid, "KILL");
}

#[cfg(unix)]
fn signal_group(pid: u32, sig: &str) {
    // The child is the leader of its own process group (pgid == pid); the
    // pid cannot be reused before our waiter thread reaps it. Without a
    // `kill` executable (a minimal container without procps) the shell's
    // builtin does it; dash's takes no `--`.
    let quiet = |c: &mut Command| {
        c.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).status()
    };
    if quiet(Command::new("kill").args([&format!("-{sig}"), "--", &format!("-{pid}")])).is_err() {
        let _ = quiet(Command::new("sh").args(["-c", &format!("kill -{sig} -{pid}")]));
    }
}

#[cfg(not(unix))]
fn kill_group(_pid: u32) {}

#[cfg(not(unix))]
fn signal_group(_pid: u32, _sig: &str) {}

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

    /// A timeout kills the child's descendants too: a grandchild that holds
    /// the output pipes (clang's `cc1` under the driver) would otherwise keep
    /// the readers, and the run, blocked until it exits.
    #[cfg(unix)]
    #[test]
    fn times_out_and_kills_the_group() {
        let o = run(Command::new("sh").args(["-c", "sleep 30; :"]), Duration::from_millis(200))
            .unwrap();
        assert!(o.timed_out());
        assert!(o.elapsed < Duration::from_secs(10), "{:?}", o.elapsed);
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

    /// The shutdown kills a registered child's whole group and waits for its
    /// reaping; a spawn in flight when it starts is killed as it registers;
    /// later spawns fail. (A private table: the global one is shared with the
    /// other tests of this process.)
    #[cfg(unix)]
    #[test]
    fn shutdown_kills_and_reaps_live_children() {
        use std::os::unix::process::{CommandExt as _, ExitStatusExt as _};
        static TABLE: LiveChildren = LiveChildren::new();
        let start = |spawning: Spawning<'static>| {
            let mut c = Command::new("sh");
            c.args(["-c", "sleep 30; :"]).process_group(0);
            let mut child = c.spawn().unwrap();
            let live = spawning.spawned(Some(child.id()), Stop::Kill);
            thread::spawn(move || {
                let s = child.wait().unwrap();
                drop(live);
                s
            })
        };
        let first = start(TABLE.begin_spawn().unwrap());
        let late = TABLE.begin_spawn().unwrap();
        let late = thread::spawn(move || {
            thread::sleep(Duration::from_millis(300));
            start(late).join().unwrap()
        });
        let t = Instant::now();
        assert_eq!(TABLE.shutdown(Duration::from_secs(20), TERM_GRACE), 1);
        assert!(t.elapsed() < Duration::from_secs(10), "{:?}", t.elapsed());
        assert_eq!(first.join().unwrap().signal(), Some(9));
        assert_eq!(late.join().unwrap().signal(), Some(9));
        assert_eq!(TABLE.lock().pids, Vec::new());
        let refused = TABLE.begin_spawn().map(|_| ()).unwrap_err();
        assert_eq!(refused.kind(), io::ErrorKind::Interrupted);
    }

    /// Starts `script` (under `sh -c`, after it made `<dir>/ready`) as a
    /// `run_step` cargo step in `table`, on a thread, and waits until it is
    /// registered and ready.
    #[cfg(unix)]
    fn start_step(
        table: &'static LiveChildren,
        dir: &Path,
        script: &str,
    ) -> thread::JoinHandle<(ExitStatus, String)> {
        let ready = dir.join("ready");
        let mut c = Command::new("sh");
        c.args([
            "-c",
            &format!("{script}; : > '{}'; while :; do sleep 0.05; done", ready.display()),
        ]);
        let step = thread::spawn(move || run_step_in(Some(table), &mut c, true, None).unwrap());
        let t = Instant::now();
        while !(ready.exists() && table.lock().pids.len() == 1) {
            assert!(t.elapsed() < Duration::from_secs(10), "the step never got ready");
            thread::sleep(Duration::from_millis(10));
        }
        step
    }

    /// A cargo step is in the live-children table from spawn to reap (as a
    /// `Stop::Term` group leader), and a shutdown stops it with `SIGTERM`
    /// first: its `trap` runs, and the stderr it wrote is still copied and
    /// returned. (A private table, as above.)
    #[cfg(unix)]
    #[test]
    fn run_step_is_registered_and_terminated() {
        static TABLE: LiveChildren = LiveChildren::new();
        let dir = scratch("step-term");
        let step = start_step(&TABLE, &dir, "trap 'echo term >&2; exit 7' TERM; echo up >&2");
        assert_eq!(TABLE.lock().pids.first().map(|&(_, stop)| stop), Some(Stop::Term));
        let t = Instant::now();
        assert_eq!(TABLE.shutdown(Duration::from_secs(20), Duration::from_secs(10)), 1);
        assert!(t.elapsed() < Duration::from_secs(8), "{:?}", t.elapsed());
        let (status, stderr) = step.join().unwrap();
        assert_eq!(status.code(), Some(7), "{status:?}");
        // (The shell may report its killed `sleep` in between.)
        assert!(stderr.starts_with("up\n") && stderr.ends_with("term\n"), "{stderr:?}");
        assert_eq!(TABLE.lock().pids, Vec::new());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// A step that ignores `SIGTERM` is killed once the grace is over.
    #[cfg(unix)]
    #[test]
    fn run_step_ignoring_term_is_killed() {
        use std::os::unix::process::ExitStatusExt as _;
        static TABLE: LiveChildren = LiveChildren::new();
        let dir = scratch("step-kill");
        let step = start_step(&TABLE, &dir, "trap '' TERM");
        let t = Instant::now();
        assert_eq!(TABLE.shutdown(Duration::from_secs(20), Duration::from_millis(300)), 1);
        assert!(t.elapsed() < Duration::from_secs(10), "{:?}", t.elapsed());
        assert_eq!(step.join().unwrap().0.signal(), Some(9));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// Without a table (no signal thread) a step just runs: its status and
    /// stderr come back.
    #[test]
    fn run_step_without_table() {
        let mut c = Command::new("sh");
        c.args(["-c", "echo e >&2; exit 3"]);
        let (status, stderr) = run_step_in(None, &mut c, true, None).unwrap();
        assert_eq!((status.code(), stderr.as_str()), (Some(3), "e\n"));
        let (status, stderr) = run_step_in(None, &mut Command::new("true"), false, None).unwrap();
        assert!(status.success() && stderr.is_empty());
        // Hidden lines are still captured.
        let mut c = Command::new("sh");
        c.args(["-c", "echo a >&2; echo 'warning: x hide me' >&2; echo >&2; echo b >&2"]);
        let (_, stderr) = run_step_in(None, &mut c, true, Some("hide me")).unwrap();
        assert_eq!(stderr, "a\nwarning: x hide me\n\nb\n");
    }

    #[test]
    fn measured_run_times_out_and_kills_the_group() {
        let dir = scratch("timeout");
        let probe = RssProbe::detect(&dir);
        let mut c = Command::new("sleep");
        c.arg("30");
        let m =
            run_measured(&c, &probe, &dir.join("rss"), Duration::from_millis(200), None).unwrap();
        assert!(m.status.is_none());
        assert!(m.wall < Duration::from_secs(10), "{m:?}");
        assert_eq!(m.max_rss_kb, None);
        assert!(m.as_output().describe_end().starts_with("timed out"));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// Core dumps are off under the wrapper, a crash still ends the
    /// process with its own signal, and the program's arguments, working
    /// directory and environment are kept (WP-1888).
    #[cfg(unix)]
    #[test]
    fn without_core_dumps_keeps_the_signal() {
        use std::os::unix::process::ExitStatusExt as _;
        let dir = scratch("no-core");
        let mut c = Command::new("sh");
        c.args(["-c", "printf '%s|%s|' \"$1\" \"$KILN_NC\"; pwd; ulimit -c", "x", "a b"])
            .current_dir(&dir)
            .env("KILN_NC", "v");
        let o = run(&mut without_core_dumps(&c), Duration::from_secs(30)).unwrap();
        let cwd = dir.canonicalize().unwrap();
        assert_eq!(o.stdout_str(), format!("a b|v|{}\n0\n", cwd.display()));
        let mut c = Command::new("sh");
        c.args(["-c", "kill -SEGV $$"]);
        let o = run(&mut without_core_dumps(&c), Duration::from_secs(30)).unwrap();
        assert_eq!(o.status.and_then(|s| s.signal()), Some(11), "{}", o.describe_end());
        assert_eq!(o.describe_end(), "killed by signal 11");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// The entries of `/proc/self/fd` and `/proc/self/task`: this process's
    /// descriptors and threads.
    #[cfg(target_os = "linux")]
    fn fds_and_threads() -> (usize, usize) {
        let n = |d: &str| std::fs::read_dir(d).map_or(0, Iterator::count);
        (n("/proc/self/fd"), n("/proc/self/task"))
    }

    /// [`child_runs_keep_no_descriptors_inner`] in a process of its own:
    /// this test binary again, one test thread (no other test opens or
    /// closes anything meanwhile, so a count that grew is a leak) and no
    /// jobserver (2000 token waits on a shared pool took minutes).
    #[cfg(target_os = "linux")]
    #[test]
    fn child_runs_keep_no_descriptors() {
        let inner = "util::process::tests::child_runs_keep_no_descriptors_inner";
        let o = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", inner, "--ignored", "--test-threads=1", "--nocapture"])
            .env_remove("MAKEFLAGS")
            .output()
            .unwrap();
        let out = String::from_utf8_lossy(&o.stdout);
        let err = String::from_utf8_lossy(&o.stderr);
        assert!(o.status.success() && out.contains("1 passed"), "{out}\n{err}");
    }

    /// 2000 children started by every route of the harness's child-process
    /// path (capped and wrapped `run`, `run_with_input`, `run_streaming`,
    /// `run_measured`, a spawn that fails, a timeout) leave this process's
    /// descriptor and thread counts no higher than they were: no pipe,
    /// child handle or reader thread outlives its run (WP-2780, WP-3420).
    #[cfg(target_os = "linux")]
    #[test]
    #[ignore = "run by child_runs_keep_no_descriptors in a process of its own"]
    fn child_runs_keep_no_descriptors_inner() {
        const RUNS: usize = 2000;
        let dir = scratch("fds");
        let rss_file = dir.join("rss.txt");
        let cap = MemLimit::from_mib(DEFAULT_MEM_LIMIT_MIB);
        let t = Duration::from_secs(60);
        let one = |i: usize| match i % 8 {
            0..=3 => {
                let o = run(&mut limited(&without_core_dumps(&Command::new("true")), cap), t);
                assert!(o.unwrap().success());
            }
            4 => {
                let mut c = Command::new("sh");
                c.args(["-c", "cat >/dev/null; echo err >&2"]);
                let o = run_with_input(&mut limited(&c, cap), Some(b"input\n"), t).unwrap();
                assert_eq!(o.stderr_str(), "err\n");
            }
            5 => {
                let read = |mut p: std::process::ChildStdout| {
                    let mut s = String::new();
                    let _ = p.read_to_string(&mut s);
                    s
                };
                let (o, s) = run_streaming(Command::new("echo").arg("x"), t, read).unwrap();
                assert!(o.success() && s.as_deref() == Some("x\n"));
            }
            6 => {
                let m =
                    run_measured(&Command::new("true"), &RssProbe::Unavailable, &rss_file, t, cap);
                assert!(m.unwrap().status.is_some_and(|s| s.success()));
            }
            _ if i % 80 == 7 => {
                let o = run(Command::new("sleep").arg("5"), Duration::from_millis(20)).unwrap();
                assert!(o.timed_out());
            }
            _ => assert!(run(&mut Command::new(dir.join("no-such-program")), t).is_err()),
        };
        // The first runs initialize what is set up once per process.
        (0..8).for_each(&one);
        let (fds, threads) = fds_and_threads();
        (0..RUNS).for_each(&one);
        let (fds_after, threads_after) = fds_and_threads();
        // A run's reader thread or pipe may still be closing when the
        // first count is taken (the warm-up's timeout run): the counts may
        // drop, and a leak is a count that grew.
        assert!(fds_after <= fds, "descriptors before {fds} and after {RUNS} runs {fds_after}");
        assert!(
            threads_after <= threads,
            "threads before {threads} and after {RUNS} runs {threads_after}"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// No crashing child leaves a `core` in its working directory (WP-2762):
    /// not under [`without_core_dumps`], not under a [`MemLimit`] (every
    /// capped compile and run), and not as a child of the xtask that
    /// [`exec_without_core_dumps`] executes (here a shell in its place,
    /// whose own child crashes: the limit is inherited).
    #[cfg(unix)]
    #[test]
    fn crashing_children_leave_no_core() {
        use std::os::unix::process::ExitStatusExt as _;
        let dir = scratch("crash");
        let crash = || {
            let mut c = Command::new("sh");
            c.args(["-c", "kill -SEGV $$"]).current_dir(&dir);
            c
        };
        let t = Duration::from_secs(30);
        let cap = MemLimit { mib: DEFAULT_MEM_LIMIT_MIB };
        for mut c in [no_core_wrapper(&crash()), without_core_dumps(&crash()), cap.wrap(&crash())] {
            let o = run(&mut c, t).unwrap();
            assert_eq!(o.status.and_then(|s| s.signal()), Some(11), "{}", o.describe_end());
        }
        let mut xtask = Command::new("sh");
        xtask
            .args([
                "-c",
                "sh -c 'kill -SEGV $$'; printf '%s %s' \"$?\" \"$KILN_XTASK_NO_CORE\"; ulimit -c",
            ])
            .current_dir(&dir);
        let o = run(&mut core_off_reexec(&xtask), t).unwrap();
        assert_eq!(o.stdout_str(), "139 10\n", "{}", o.stderr_str());
        let left: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok()?.file_name().into_string().ok())
            .filter(|n| n == "core" || n.starts_with("core."))
            .collect();
        assert!(left.is_empty(), "core dumps left in {}: {left:?}", dir.display());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// `/proc/<pid>/limits`' core row: soft limit 0 or not.
    #[test]
    fn reads_the_core_limit() {
        let row = |soft: &str| {
            format!(
                "Limit                     Soft Limit           Hard Limit           Units     \n\
                 Max file size             unlimited            unlimited            bytes     \n\
                 Max core file size        {soft:<20} unlimited            bytes     \n"
            )
        };
        assert!(core_limit_is_zero(&row("0")));
        assert!(!core_limit_is_zero(&row("unlimited")));
        assert!(!core_limit_is_zero(&row("1024")));
        assert!(!core_limit_is_zero(""));
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
        let o = run(
            &mut limited(&hog, MemLimit::from_mib(DEFAULT_MEM_LIMIT_MIB)),
            Duration::from_secs(30),
        )
        .unwrap();
        assert!(o.success() && !o.out_of_memory(), "{o:?}");
        // The cap applies under the RSS probe, too.
        let dir = scratch("hog");
        let probe = RssProbe::detect(&dir);
        let m = run_measured(
            &hog,
            &probe,
            &dir.join("rss"),
            Duration::from_secs(30),
            MemLimit::from_mib(256),
        )
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
        assert!(out_of_memory(
            exit(2),
            "kiln-mem-limit: 1: exec: /bin/x: Cannot allocate memory\n"
        ));
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
        assert_eq!(
            kiln_stack_reservation_mib(&["-fkiln-stack-size=100k"]),
            4,
            "raised to the minimum"
        );
        assert_eq!(kiln_stack_reservation_mib(&["-fkiln-stack-size=5242881"]), 6, "rounded up");
        // The last valid one wins, as in kiln; an invalid one is skipped.
        assert_eq!(
            kiln_stack_reservation_mib(&["-fkiln-stack-size=64M", "-fkiln-stack-size=x"]),
            64
        );
        assert_eq!(
            kiln_stack_reservation_mib(&["-fkiln-stack-size=64M", "-fkiln-stack-size=32M"]),
            32
        );
        assert_eq!(kiln_stack_reservation_mib(&["-fkiln-stack-size=0"]), 1024);
    }

    #[test]
    fn shown_quotes() {
        let mut c = Command::new("cc");
        c.args(["-DX=a b", "plain", "it's"]);
        assert_eq!(Shown(&c).to_string(), r"cc '-DX=a b' plain 'it'\''s'");
    }
}
