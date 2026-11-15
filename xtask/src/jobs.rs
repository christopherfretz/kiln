//! GNU make jobserver client (fifo style), so the processes xtask spawns
//! share one host-wide parallelism pool with cargo and rustc
//! (`spec/harness.md` "Jobserver").
//!
//! When `MAKEFLAGS` carries `--jobserver-auth=fifo:<path>` (make 4.4's fifo
//! style; `scripts/env.sh` exports it when the shared pool is live), every
//! child started through [`crate::util::process`] holds a [`Token`] for its
//! lifetime. Like make, the process owns one *implicit* token: one child may
//! run without taking anything from the pool, every further concurrent child
//! reads one byte from the fifo and writes the same byte back when it exits.
//! `-j N` stays the number of worker threads, so it is an upper bound on
//! in-flight children; the pool can only lower it. Without a jobserver (no
//! `MAKEFLAGS`, the older `--jobserver-fds=R,W` form, which needs inherited
//! descriptors, or a fifo that cannot be opened) [`Client::acquire`] is free
//! and `-j` alone bounds the parallelism, exactly as before.
//!
//! Waiting is done by one lazily started helper thread blocked in `read` on
//! the fifo, never by the workers themselves: a worker blocked in `read`
//! could not notice that the implicit token became free in the meantime.
//! Tokens are released by `Drop`, so a panicking worker, a child killed by
//! the memory cap or the timeout, and an early `?` return all give the token
//! back. SIGINT and SIGTERM are caught (by a `signal-hook` thread, so the
//! `unsafe` of signal handling stays in that dependency): every live child of
//! `util::process` is killed and reaped, then the held and spare pool bytes
//! are written back, then the signal's default action ends the process,
//! always, within a bound, whatever the cleanup or the other threads do
//! ([`release_on_signal`]). Only SIGKILL (which no process can catch) or another uncaught
//! fatal signal loses the tokens xtask held, as it would for make or cargo;
//! `cargo xtask jobserver refill` restores the pool afterwards.

use std::fs::{File, OpenOptions};
use std::io::{Read as _, Write as _};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, Once, OnceLock, PoisonError};
use std::time::{Duration, Instant};

/// The jobserver this process inherited, if any (read from `MAKEFLAGS` once).
/// The first call also starts the SIGINT/SIGTERM thread (with or without a
/// pool: it kills the live children either way).
pub fn global() -> &'static Client {
    static GLOBAL: OnceLock<Client> = OnceLock::new();
    static SIGNALS: Once = Once::new();
    let client = GLOBAL.get_or_init(|| {
        let flags = std::env::var("MAKEFLAGS").unwrap_or_default();
        let Some(path) = fifo_from_makeflags(&flags) else { return Client::none() };
        Client::open(&path).unwrap_or_else(|e| {
            eprintln!(
                "xtask: warning: cannot use jobserver fifo {} ({e}); falling back to -j",
                path.display()
            );
            Client::none()
        })
    });
    SIGNALS.call_once(|| {
        hold_panics_while_interrupted();
        release_on_signal(client);
    });
    client
}

/// Whether the SIGINT/SIGTERM thread runs (set once it is started).
static SIGNALS_CAUGHT: AtomicBool = AtomicBool::new(false);

/// Starts the SIGINT/SIGTERM thread if it is not running yet ([`global`])
/// and says whether it runs: when it does, a child in a process group of
/// its own is still stopped by an interrupt of xtask
/// (`util::process::run_step`).
pub fn signals_caught() -> bool {
    global();
    SIGNALS_CAUGHT.load(Ordering::SeqCst)
}

/// The signal that interrupted the process (0: none yet), set by the signal
/// thread before anything else happens on the signal path.
static INTERRUPTED: AtomicI32 = AtomicI32::new(0);

/// The interrupting signal, once SIGINT or SIGTERM has arrived.
pub fn interrupted() -> Option<i32> {
    match INTERRUPTED.load(Ordering::SeqCst) {
        0 => None,
        s => Some(s),
    }
}

/// The time limits of the signal path (`spec/harness.md` "Jobserver",
/// Signals). The test binary can stretch or shrink them through
/// `KILN_XTASK_TEST_SIGNAL_MS=<reap>,<term>,<bound>` (milliseconds), so the
/// tests can hold a phase open without sleeping; release builds never read it.
#[derive(Clone, Copy, Debug)]
struct Limits {
    /// How long the cleanup waits for its killed children to be reaped
    /// before returning the tokens anyway (a child in uninterruptible sleep).
    reap: Duration,
    /// How long a cargo step gets after `SIGTERM` before `SIGKILL`.
    term: Duration,
    /// How long the whole cleanup may take before the process ends anyway
    /// (a cleanup blocked on a full stderr pipe, a lock, a stuck child).
    bound: Duration,
}

fn limits() -> Limits {
    let default = Limits {
        reap: Duration::from_secs(10),
        term: crate::util::process::TERM_GRACE,
        bound: Duration::from_secs(15),
    };
    #[cfg(test)]
    if let Ok(v) = std::env::var("KILN_XTASK_TEST_SIGNAL_MS") {
        let ms: Vec<u64> = v.split(',').filter_map(|s| s.parse().ok()).collect();
        if let [reap, term, bound] = ms[..] {
            return Limits {
                reap: Duration::from_millis(reap),
                term: Duration::from_millis(term),
                bound: Duration::from_millis(bound),
            };
        }
    }
    default
}

/// A signal this soon after the first is the same interruption delivered
/// twice (`timeout(1)` signals its child and then its whole process group),
/// not a second request.
const DUPLICATE_WINDOW: Duration = Duration::from_millis(500);

/// Starts the signal thread (`spec/harness.md` "Jobserver", Signals). On the
/// first SIGINT or SIGTERM it records the signal and starts the cleanup on
/// a thread of its own: kill and reap every live child
/// (`util::process::kill_live_children`), then return every pool byte the
/// process holds (after the children are dead, so the pool is never
/// oversubscribed by an orphan, and exactly once), then report both on
/// stderr. Whatever the cleanup does (finishes, panics, blocks on a full
/// stderr pipe or a stuck child), the signal thread ends the process with
/// the signal's default action once it is over or after [`Limits::bound`]
/// (exit status 128 + signal if that cannot be emulated). A second signal
/// during the cleanup ends the process at once, after `SIGKILL` to every live
/// child group. Nothing on this thread can panic: its messages ignore write
/// errors (stderr may be a pipe whose reader is gone, which is how WP-1436's
/// hang happened: the message panicked, the thread died, and the main thread
/// waited for it forever).
#[cfg(unix)]
fn release_on_signal(client: &'static Client) {
    use signal_hook::consts::{SIGINT, SIGTERM};
    let mut signals = match signal_hook::iterator::Signals::new([SIGINT, SIGTERM]) {
        Ok(s) => s,
        Err(e) => {
            say(format_args!(
                "xtask: warning: cannot catch SIGINT/SIGTERM ({e}); an interrupted run \
                 will neither kill its children nor return its jobserver tokens"
            ));
            return;
        }
    };
    let spawned = std::thread::Builder::new().name(SIGNAL_THREAD.into()).spawn(move || {
        // `forever` only ends when the handle is closed, which nothing does.
        let Some(sig) = signals.forever().next() else { return };
        INTERRUPTED.store(sig, Ordering::SeqCst);
        let limits = limits();
        let start = Instant::now();
        let (tx, rx) = std::sync::mpsc::channel::<()>();
        let cleanup = move || {
            let killed = crate::util::process::kill_live_children(limits.reap, limits.term);
            let returned = client.release_all();
            if killed > 0 {
                say(format_args!("xtask: signal {sig}: killed {killed} child process(es)"));
            }
            if returned > 0 {
                say(format_args!("xtask: signal {sig}: returned {returned} jobserver token(s)"));
            }
            let _ = tx.send(());
        };
        let worker = std::thread::Builder::new().name(CLEANUP_THREAD.into());
        if let Err(e) = worker.spawn(cleanup) {
            // No thread for it: the cleanup runs here, unbounded but
            // panic-free; the main thread's own deadline still holds.
            let _ = crate::util::process::kill_live_children(limits.reap, limits.term);
            client.release_all();
            say_bounded(format!("xtask: warning: cannot start the cleanup thread ({e})"));
            die(sig);
        }
        loop {
            match rx.recv_timeout(Duration::from_millis(50)) {
                // Done, or the cleanup panicked (its sender is gone).
                Ok(()) | Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            }
            if start.elapsed() >= limits.bound {
                crate::util::process::kill_live_children_now();
                client.release_all();
                say_bounded(format!(
                    "xtask: signal {sig}: cleanup still running after {:.1}s; exiting anyway",
                    limits.bound.as_secs_f64()
                ));
                break;
            }
            if let Some(again) = signals.pending().next()
                && start.elapsed() >= DUPLICATE_WINDOW
            {
                crate::util::process::kill_live_children_now();
                client.release_all();
                say_bounded(format!("xtask: signal {again}: exiting without waiting for cleanup"));
                die(again);
            }
        }
        die(sig);
    });
    match spawned {
        Ok(_) => SIGNALS_CAUGHT.store(true, Ordering::SeqCst),
        Err(e) => {
            say(format_args!("xtask: warning: cannot start the jobserver signal thread ({e})"));
        }
    }
}

#[cfg(not(unix))]
fn release_on_signal(_: &'static Client) {}

/// The names of the threads of the signal path.
const SIGNAL_THREAD: &str = "jobserver-signals";
const CLEANUP_THREAD: &str = "jobserver-cleanup";

/// One line on stderr that never panics: on the signal path stderr may be a
/// pipe whose reader is gone (EPIPE), where `eprintln!` panics.
fn say(args: std::fmt::Arguments<'_>) {
    let _ = writeln!(std::io::stderr(), "{args}");
}

/// [`say`] on a thread of its own, waiting for it at most 100 ms: the
/// signal thread must not block on a full stderr pipe.
fn say_bounded(line: String) {
    let (tx, rx) = std::sync::mpsc::channel();
    let spawned = std::thread::Builder::new().spawn(move || {
        say(format_args!("{line}"));
        let _ = tx.send(());
    });
    if spawned.is_ok() {
        let _ = rx.recv_timeout(Duration::from_millis(100));
    }
}

/// Ends the process as `sig` would by default (so the parent sees "killed
/// by SIGTERM"); if that does not end it, `_exit(128 + sig)` without running
/// exit handlers, which `std::process::exit` would do and which can block
/// behind another thread's exit.
#[cfg(unix)]
fn die(sig: i32) -> ! {
    let _ = signal_hook::low_level::emulate_default_handler(sig);
    signal_hook::low_level::exit(128 + sig)
}

#[cfg(not(unix))]
fn die(sig: i32) -> ! {
    std::process::exit(128 + sig)
}

/// For a thread that finds the process interrupted ([`interrupted`]): waits
/// for the signal thread to end the process, and ends it itself if that has
/// not happened well after [`Limits::bound`], so no thread is left waiting
/// forever. The cargo steps of `util::process::run_step`, `main` once a
/// command returns, and a thread panicking during the cleanup (its panic
/// message would mostly be the dead stderr pipe) end here.
pub fn exit_interrupted() -> ! {
    let deadline = Instant::now() + limits().bound + Duration::from_secs(5);
    loop {
        let now = Instant::now();
        if now >= deadline {
            break;
        }
        std::thread::park_timeout(deadline - now);
    }
    die(interrupted().unwrap_or(15))
}

/// `main`'s exit: `code`, unless the process is being interrupted, in which
/// case the signal thread ends it (with the signal's status, after the
/// cleanup) instead of `main` racing it to `exit`.
pub fn finish(code: ExitCode) -> ExitCode {
    if interrupted().is_some() {
        exit_interrupted();
    }
    code
}

/// Makes a panic of any other thread during the cleanup wait for the
/// process to end ([`exit_interrupted`]) instead of unwinding into `main`,
/// which would exit with 101 before the children are reaped and the tokens
/// returned. Outside an interruption the previous hook runs as before.
fn hold_panics_while_interrupted() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let own = std::thread::current()
            .name()
            .is_some_and(|n| n == SIGNAL_THREAD || n == CLEANUP_THREAD);
        if interrupted().is_some() && !own {
            exit_interrupted();
        }
        previous(info);
    }));
}

/// The fifo of the last `--jobserver-auth=` word of `MAKEFLAGS` when it is
/// the `fifo:<path>` form (make passes the last one as authoritative).
pub fn fifo_from_makeflags(flags: &str) -> Option<PathBuf> {
    flags
        .split_ascii_whitespace()
        .filter_map(|w| w.strip_prefix("--jobserver-auth="))
        .next_back()?
        .strip_prefix("fifo:")
        .filter(|p| !p.is_empty())
        .map(PathBuf::from)
}

/// A jobserver connection, or the "no jobserver" stand-in.
#[derive(Debug)]
pub struct Client {
    pool: Option<Arc<Pool>>,
}

#[derive(Debug)]
struct Pool {
    path: PathBuf,
    fifo: File,
    state: Mutex<State>,
    /// Workers waiting for a token.
    workers: Condvar,
    /// The helper waiting for a read request.
    helper: Condvar,
}

// Four independent flags of one mutex-guarded state machine.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug)]
struct State {
    implicit_free: bool,
    /// Pool bytes read for (or handed over to) waiting workers.
    spare: Vec<u8>,
    waiters: usize,
    /// Reads the helper still has to do (including one in flight).
    requested: usize,
    helper_started: bool,
    /// The fifo failed; acquisition degrades to `-j` alone.
    broken: bool,
    /// Pool bytes held by live [`Token`]s (returned by
    /// [`Client::release_all`] if the process is interrupted).
    held: Vec<u8>,
    /// [`Client::release_all`] ran: every byte went back, so tokens dropped
    /// later return nothing and new ones are free (the process is exiting).
    released: bool,
    /// The helper is in (or just back from) a `read` of the fifo, so it may
    /// hold a byte nobody has accounted for yet.
    reading: bool,
}

/// A held parallelism slot; dropping it releases it.
#[derive(Debug)]
pub struct Token {
    kind: Kind,
    pool: Option<Arc<Pool>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    /// No jobserver (or a broken one): nothing to give back.
    Free,
    Implicit,
    Pool(u8),
}

impl Client {
    /// No jobserver: [`Client::acquire`] never waits.
    pub fn none() -> Self {
        Self { pool: None }
    }

    /// Connects to the fifo at `path` (opened read+write, so neither end
    /// ever sees EOF or EPIPE).
    pub fn open(path: &Path) -> std::io::Result<Self> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::FileTypeExt as _;
            if !std::fs::metadata(path)?.file_type().is_fifo() {
                return Err(std::io::Error::other("not a fifo"));
            }
        }
        let fifo = OpenOptions::new().read(true).write(true).open(path)?;
        let state = State {
            implicit_free: true,
            spare: Vec::new(),
            waiters: 0,
            requested: 0,
            helper_started: false,
            broken: false,
            held: Vec::new(),
            released: false,
            reading: false,
        };
        Ok(Self {
            pool: Some(Arc::new(Pool {
                path: path.to_owned(),
                fifo,
                state: Mutex::new(state),
                workers: Condvar::new(),
                helper: Condvar::new(),
            })),
        })
    }

    /// `fifo:<path>`, or `None` without a jobserver.
    pub fn describe(&self) -> Option<String> {
        self.pool.as_ref().map(|p| format!("fifo:{}", p.path.display()))
    }

    /// Blocks until a slot is available: the implicit token if it is free,
    /// else one byte from the pool. Without a jobserver it returns at once.
    pub fn acquire(&self) -> Token {
        let Some(pool) = &self.pool else { return Token { kind: Kind::Free, pool: None } };
        let token = |kind| Token { kind, pool: Some(Arc::clone(pool)) };
        let mut st = pool.lock();
        if st.implicit_free {
            st.implicit_free = false;
            return token(Kind::Implicit);
        }
        if st.broken || st.released {
            return token(Kind::Free);
        }
        st.waiters += 1;
        st.requested += 1;
        if !st.helper_started {
            st.helper_started = true;
            let p = Arc::clone(pool);
            let spawned =
                std::thread::Builder::new().name("jobserver".into()).spawn(move || helper(&p));
            if let Err(e) = spawned {
                eprintln!(
                    "xtask: warning: cannot start jobserver helper ({e}); falling back to -j"
                );
                st.broken = true;
            }
        }
        pool.helper.notify_one();
        loop {
            // A spare byte was read (or handed over) for a waiter: take it
            // before the implicit token, otherwise it would be stranded in
            // `spare` with nobody waiting for it — a pool token this process
            // keeps but does not use, and leaks on a normal exit.
            if let Some(b) = st.spare.pop() {
                st.waiters -= 1;
                st.held.push(b);
                return token(Kind::Pool(b));
            }
            if st.implicit_free {
                st.implicit_free = false;
                st.waiters -= 1;
                // The read requested for us is no longer needed.
                st.requested = st.requested.saturating_sub(1);
                return token(Kind::Implicit);
            }
            if st.broken || st.released {
                st.waiters -= 1;
                return token(Kind::Free);
            }
            st = pool.workers.wait(st).unwrap_or_else(PoisonError::into_inner);
        }
    }

    /// Writes every pool byte this process holds (by live tokens, or read
    /// for waiting workers) back to the fifo, and returns how many. After
    /// it, dropping a token returns nothing and acquiring is free: it is
    /// the last act of an interrupted process. Idempotent.
    pub fn release_all(&self) -> usize {
        let Some(pool) = &self.pool else { return 0 };
        let bytes: Vec<u8> = {
            let mut st = pool.lock();
            st.released = true;
            let mut v = std::mem::take(&mut st.held);
            v.append(&mut st.spare);
            v
        };
        pool.workers.notify_all();
        for &b in &bytes {
            pool.give_back(b);
        }
        // The helper may be blocked in `read`, and the bytes just written
        // back wake it: the one it takes goes straight back too, but only
        // if the process lives that long. Wait (briefly: with an empty pool
        // it stays blocked) until it has.
        let deadline = Instant::now() + HELPER_SETTLE;
        let mut st = pool.lock();
        while st.reading {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                break;
            }
            st = pool.workers.wait_timeout(st, left).unwrap_or_else(PoisonError::into_inner).0;
        }
        bytes.len()
    }
}

impl Pool {
    /// The state is a handful of counters updated atomically under the lock,
    /// so a poisoned lock still holds a consistent value.
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Returns `byte` to the shared pool.
    fn give_back(&self, byte: u8) {
        if let Err(e) = (&self.fifo).write_all(&[byte]) {
            eprintln!(
                "xtask: warning: lost a jobserver token: write to {} failed: {e}",
                self.path.display()
            );
        }
    }
}

/// How long [`Client::release_all`] waits for a helper `read` in flight to
/// put back the byte it may have taken.
const HELPER_SETTLE: Duration = Duration::from_millis(200);

/// Reads one pool byte per request; a byte nobody waits for any more goes
/// straight back to the pool. After [`Client::release_all`] it reads no more.
fn helper(pool: &Pool) {
    loop {
        {
            let mut st = pool.lock();
            while st.requested == 0 || st.released {
                st = pool.helper.wait(st).unwrap_or_else(PoisonError::into_inner);
            }
            st.reading = true;
        }
        let mut byte = [0u8; 1];
        let r = (&pool.fifo).read_exact(&mut byte);
        let mut st = pool.lock();
        st.requested = st.requested.saturating_sub(1);
        st.reading = false;
        match r {
            Ok(()) if st.waiters > st.spare.len() && !st.released => {
                st.spare.push(byte[0]);
                pool.workers.notify_all();
            }
            Ok(()) => {
                // Under the lock, so `release_all` sees `reading` clear only
                // once the byte is back.
                pool.give_back(byte[0]);
                pool.workers.notify_all();
            }
            Err(e) => {
                eprintln!(
                    "xtask: warning: jobserver read from {} failed ({e}); falling back to -j",
                    pool.path.display()
                );
                st.broken = true;
                pool.workers.notify_all();
                return;
            }
        }
    }
}

impl Drop for Token {
    fn drop(&mut self) {
        let Some(pool) = &self.pool else { return };
        match self.kind {
            Kind::Free => {}
            Kind::Implicit => {
                pool.lock().implicit_free = true;
                pool.workers.notify_all();
            }
            Kind::Pool(b) => {
                let mut st = pool.lock();
                if st.released {
                    // `release_all` already wrote this byte back.
                    return;
                }
                if let Some(i) = st.held.iter().position(|&h| h == b) {
                    st.held.swap_remove(i);
                }
                if st.waiters > st.spare.len() {
                    // Hand it to a waiting worker directly; its read request
                    // is then satisfied.
                    st.spare.push(b);
                    st.requested = st.requested.saturating_sub(1);
                    pool.workers.notify_all();
                } else {
                    drop(st);
                    pool.give_back(b);
                }
            }
        }
    }
}

// ----- `cargo xtask jobserver` ---------------------------------------------

const JOBSERVER_USAGE: &str = "\
usage: cargo xtask jobserver <status|refill> [--fifo <path>] [--jobs <n>]

  status   print the idle tokens in the pool (read without blocking and put
           straight back)
  refill   top the pool up to <n> idle tokens: count them with non-blocking
           reads, write them back plus the missing ones with non-blocking
           writes. Only meaningful while no client holds tokens (no build or
           test running): tokens in use come back on top. A pool above <n>
           is reported and left alone.

  --fifo <path>  default: MAKEFLAGS' --jobserver-auth=fifo:, else
                 $KILN_JOBSERVER_FIFO, else /run/kiln/jobserver.fifo
  --jobs <n>     the pool size; default $KILN_JOBS, else 20
";

/// `cargo xtask jobserver status|refill` (`spec/harness.md` "Jobserver").
pub fn main(args: impl IntoIterator<Item = String>) -> ExitCode {
    match jobserver_cmd(args) {
        Ok(Some(line)) => {
            println!("{line}");
            ExitCode::SUCCESS
        }
        Ok(None) => {
            print!("{JOBSERVER_USAGE}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("xtask jobserver: {e}\n\n{JOBSERVER_USAGE}");
            ExitCode::from(2)
        }
    }
}

fn jobserver_cmd(args: impl IntoIterator<Item = String>) -> Result<Option<String>, String> {
    let mut cmd = None;
    let mut fifo = None;
    let mut jobs = None;
    let mut it = args.into_iter();
    while let Some(arg) = it.next() {
        let (flag, inline) = match arg.split_once('=') {
            Some((f, v)) if f.starts_with("--") => (f.to_owned(), Some(v.to_owned())),
            _ => (arg.clone(), None),
        };
        let mut value = |name: &str| -> Result<String, String> {
            inline.clone().or_else(|| it.next()).ok_or_else(|| format!("`{name}` requires a value"))
        };
        match flag.as_str() {
            "-h" | "--help" => return Ok(None),
            "--fifo" => fifo = Some(PathBuf::from(value("--fifo")?)),
            "--jobs" => {
                let v = value("--jobs")?;
                jobs = Some(v.parse::<usize>().map_err(|_| format!("bad --jobs `{v}`"))?);
            }
            "status" | "refill" if cmd.is_none() => cmd = Some(flag),
            f => return Err(format!("unknown argument `{f}`")),
        }
    }
    let Some(cmd) = cmd else { return Err("missing command (status or refill)".into()) };
    let fifo = fifo.unwrap_or_else(default_fifo);
    let jobs = match jobs {
        Some(n) => n,
        None => match std::env::var("KILN_JOBS") {
            Ok(v) => v.parse().map_err(|_| format!("bad KILN_JOBS `{v}`"))?,
            Err(_) => DEFAULT_POOL,
        },
    };
    let target = if cmd == "refill" { Some(jobs) } else { None };
    let r = refill(&fifo, target).map_err(|e| format!("{}: {e}", fifo.display()))?;
    let at = fifo.display();
    Ok(Some(match target {
        None => format!("jobserver {at}: {} idle token(s) of {jobs}", r.idle),
        Some(_) if r.added > 0 => {
            format!("jobserver {at}: {} idle, added {}: now {jobs}", r.idle, r.added)
        }
        Some(_) if r.idle > jobs => {
            format!("jobserver {at}: {} idle, above {jobs}; left alone", r.idle)
        }
        Some(_) => format!("jobserver {at}: {} idle; full", r.idle),
    }))
}

/// The pool size of `scripts/jobserver.sh` when `KILN_JOBS` is unset.
const DEFAULT_POOL: usize = 20;

fn default_fifo() -> PathBuf {
    let flags = std::env::var("MAKEFLAGS").unwrap_or_default();
    fifo_from_makeflags(&flags)
        .or_else(|| std::env::var_os("KILN_JOBSERVER_FIFO").map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("/run/kiln/jobserver.fifo"))
}

#[derive(Debug, PartialEq, Eq)]
struct Refill {
    /// Idle tokens found.
    idle: usize,
    /// Tokens written on top of the ones put back.
    added: usize,
}

/// `O_NONBLOCK` for `OpenOptionsExt::custom_flags` (a plain constant; no
/// `libc` dependency for one number).
#[cfg(all(
    any(target_os = "linux", target_os = "android"),
    not(any(
        target_arch = "mips",
        target_arch = "mips64",
        target_arch = "sparc",
        target_arch = "sparc64",
        target_arch = "powerpc",
        target_arch = "powerpc64"
    ))
))]
const O_NONBLOCK: Option<i32> = Some(0o4000);
#[cfg(any(
    target_os = "macos",
    target_os = "ios",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd",
    target_os = "dragonfly"
))]
const O_NONBLOCK: Option<i32> = Some(0x0004);
#[cfg(not(any(
    all(
        any(target_os = "linux", target_os = "android"),
        not(any(
            target_arch = "mips",
            target_arch = "mips64",
            target_arch = "sparc",
            target_arch = "sparc64",
            target_arch = "powerpc",
            target_arch = "powerpc64"
        ))
    ),
    target_os = "macos",
    target_os = "ios",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd",
    target_os = "dragonfly"
)))]
const O_NONBLOCK: Option<i32> = None;

/// Drains the fifo without blocking, writes the bytes back, and with
/// `Some(target)` adds `+` bytes up to `target` idle tokens.
fn refill(path: &Path, target: Option<usize>) -> std::io::Result<Refill> {
    use std::io::ErrorKind;
    #[cfg(unix)]
    {
        use std::os::unix::fs::FileTypeExt as _;
        if !std::fs::metadata(path)?.file_type().is_fifo() {
            return Err(std::io::Error::other("not a fifo"));
        }
    }
    let Some(flag) = O_NONBLOCK else {
        return Err(std::io::Error::other("non-blocking open is not supported on this platform"));
    };
    let mut opts = OpenOptions::new();
    opts.read(true).write(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::custom_flags(&mut opts, flag);
    #[cfg(not(unix))]
    let _ = flag;
    let mut f = opts.open(path)?;
    let mut got = Vec::new();
    let mut buf = [0u8; 64];
    loop {
        match f.read(&mut buf) {
            // Opened read+write, so there is always a writer: 0 cannot be
            // EOF, but stop rather than spin.
            Ok(0) => break,
            Ok(n) => got.extend_from_slice(&buf[..n]),
            Err(e) if e.kind() == ErrorKind::WouldBlock => break,
            Err(e) if e.kind() == ErrorKind::Interrupted => {}
            Err(e) => {
                // Put back what was taken before failing.
                let _ = f.write_all(&got);
                return Err(e);
            }
        }
    }
    let idle = got.len();
    let added = target.map_or(0, |t| t.saturating_sub(idle));
    got.resize(idle + added, b'+');
    f.write_all(&got)?;
    Ok(Refill { idle, added })
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    #[test]
    fn parses_makeflags() {
        let f = |s: &str| fifo_from_makeflags(s);
        assert_eq!(f(""), None);
        assert_eq!(f("-j8"), None);
        assert_eq!(
            f(" -j8 --jobserver-auth=fifo:/run/kiln/jobserver.fifo"),
            Some(PathBuf::from("/run/kiln/jobserver.fifo"))
        );
        assert_eq!(f("--jobserver-auth=fifo:/a --jobserver-auth=fifo:/b"), Some("/b".into()));
        // The descriptor forms need inherited fds: ignored.
        assert_eq!(f("-j --jobserver-auth=3,4"), None);
        assert_eq!(f("--jobserver-fds=3,4 -j"), None);
        assert_eq!(f("--jobserver-auth=fifo:"), None);
    }

    #[test]
    fn no_jobserver_is_free() {
        let c = Client::none();
        assert_eq!(c.describe(), None);
        let toks: Vec<Token> = (0..16).map(|_| c.acquire()).collect();
        assert!(toks.iter().all(|t| t.kind == Kind::Free));
    }

    /// Removes the test fifo's directory.
    struct Cleanup(PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn fifo(tag: &str, tokens: usize) -> (PathBuf, File, Cleanup) {
        let dir =
            std::env::temp_dir().join(format!("kiln-xtask-jobs-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("fifo");
        let ok = std::process::Command::new("mkfifo").arg(&path).status().unwrap().success();
        assert!(ok, "mkfifo failed");
        // A holder end, like scripts/jobserver.sh, primed with the pool.
        let mut holder = OpenOptions::new().read(true).write(true).open(&path).unwrap();
        holder.write_all(&vec![b'+'; tokens]).unwrap();
        (path, holder, Cleanup(dir))
    }

    /// Idle tokens in the fifo (drained non-destructively via a probe client).
    fn idle(path: &Path, expect: usize) -> usize {
        // Read exactly `expect` bytes with a blocking reader in a thread; if
        // fewer are there it would hang, so bound it with a timeout.
        let (tx, rx) = std::sync::mpsc::channel();
        let p = path.to_owned();
        std::thread::spawn(move || {
            let mut f = OpenOptions::new().read(true).write(true).open(&p).unwrap();
            let mut buf = vec![0u8; expect];
            f.read_exact(&mut buf).unwrap();
            f.write_all(&buf).unwrap();
            let _ = tx.send(());
        });
        usize::from(rx.recv_timeout(Duration::from_secs(5)).is_ok()) * expect
    }

    #[test]
    fn rejects_a_non_fifo() {
        let p = std::env::temp_dir().join(format!("kiln-xtask-jobs-plain-{}", std::process::id()));
        std::fs::write(&p, "++").unwrap();
        assert!(Client::open(&p).is_err());
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn bounds_concurrency_by_pool_plus_implicit() {
        let (path, _holder, _rm) = fifo("bound", 2);
        let c = Client::open(&path).unwrap();
        assert!(c.describe().unwrap().starts_with("fifo:"));
        let live = AtomicUsize::new(0);
        let peak = AtomicUsize::new(0);
        std::thread::scope(|s| {
            for _ in 0..8 {
                s.spawn(|| {
                    for _ in 0..5 {
                        let _t = c.acquire();
                        let n = live.fetch_add(1, Ordering::SeqCst) + 1;
                        peak.fetch_max(n, Ordering::SeqCst);
                        std::thread::sleep(Duration::from_millis(5));
                        live.fetch_sub(1, Ordering::SeqCst);
                    }
                });
            }
        });
        let peak = peak.load(Ordering::SeqCst);
        assert!(peak <= 3, "peak {peak} exceeds 2 pool tokens + the implicit one");
        assert!(peak >= 2, "peak {peak}: the pool was never used");
        // Every pool token came back.
        assert_eq!(idle(&path, 2), 2);
    }

    #[test]
    fn implicit_token_frees_a_waiter() {
        // Empty pool: only the implicit token exists.
        let (path, _holder, _rm) = fifo("implicit", 0);
        let c = Client::open(&path).unwrap();
        let first = c.acquire();
        assert_eq!(first.kind, Kind::Implicit);
        std::thread::scope(|s| {
            let h = s.spawn(|| c.acquire().kind);
            std::thread::sleep(Duration::from_millis(50));
            drop(first);
            assert_eq!(h.join().unwrap(), Kind::Implicit);
        });
    }

    #[test]
    fn release_all_returns_held_and_spare_bytes() {
        let (path, _holder, _rm) = fifo("release", 2);
        let c = Client::open(&path).unwrap();
        let _implicit = c.acquire();
        let a = c.acquire();
        let b = c.acquire();
        assert!(matches!((a.kind, b.kind), (Kind::Pool(_), Kind::Pool(_))));
        assert_eq!(refill(&path, None).unwrap().idle, 0);
        assert_eq!(c.release_all(), 2);
        assert_eq!(c.release_all(), 0, "idempotent");
        // Dropping the tokens afterwards does not return them twice, and a
        // new acquisition is free.
        drop((a, b));
        assert_eq!(c.acquire().kind, Kind::Free);
        assert_eq!(refill(&path, None).unwrap(), Refill { idle: 2, added: 0 });
    }

    #[test]
    fn refill_tops_up_without_blocking() {
        let (path, _holder, _rm) = fifo("refill", 3);
        assert_eq!(refill(&path, None).unwrap(), Refill { idle: 3, added: 0 });
        assert_eq!(refill(&path, Some(5)).unwrap(), Refill { idle: 3, added: 2 });
        assert_eq!(refill(&path, Some(5)).unwrap(), Refill { idle: 5, added: 0 });
        // Above the target: left alone.
        assert_eq!(refill(&path, Some(4)).unwrap(), Refill { idle: 5, added: 0 });
        assert_eq!(refill(&path, None).unwrap().idle, 5);
        // An empty pool is counted, not waited on.
        let (empty, _h, _rm2) = fifo("refill-empty", 0);
        assert_eq!(refill(&empty, Some(2)).unwrap(), Refill { idle: 0, added: 2 });
        let p = std::env::temp_dir().join(format!("kiln-xtask-refill-{}", std::process::id()));
        std::fs::write(&p, "").unwrap();
        assert!(refill(&p, Some(1)).is_err(), "not a fifo");
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn jobserver_cli() {
        let a = |v: &[&str]| jobserver_cmd(v.iter().map(|s| (*s).to_owned()));
        assert_eq!(a(&["--help"]), Ok(None));
        assert!(a(&[]).is_err());
        assert!(a(&["drain"]).is_err());
        assert!(a(&["status", "--jobs", "x"]).is_err());
        let (path, _holder, _rm) = fifo("cli", 1);
        let p = path.display().to_string();
        let line = a(&["refill", "--fifo", &p, "--jobs=3"]).unwrap().unwrap();
        assert!(line.ends_with("1 idle, added 2: now 3"), "{line}");
        let line = a(&["status", "--fifo", &p, "--jobs", "3"]).unwrap().unwrap();
        assert!(line.ends_with("3 idle token(s) of 3"), "{line}");
    }

    #[test]
    fn tokens_return_on_panic() {
        let (path, _holder, _rm) = fifo("panic", 1);
        let c = Client::open(&path).unwrap();
        let _implicit = c.acquire();
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let t = c.acquire();
            assert!(matches!(t.kind, Kind::Pool(b'+')));
            panic!("worker died");
        }));
        assert!(r.is_err());
        assert_eq!(idle(&path, 1), 1);
    }
}

/// SIGINT/SIGTERM delivered to a real (re-executed) process at each phase
/// (WP-1436).
#[cfg(all(test, target_os = "linux"))]
mod signal_tests;
