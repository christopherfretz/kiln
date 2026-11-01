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
//! `unsafe` of signal handling stays in that dependency): the held and spare
//! pool bytes are written back, then the signal's default action ends the
//! process. Only SIGKILL (which no process can catch) or another uncaught
//! fatal signal loses the tokens xtask held, as it would for make or cargo;
//! `cargo xtask jobserver refill` restores the pool afterwards.

use std::fs::{File, OpenOptions};
use std::io::{Read as _, Write as _};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::{Arc, Condvar, Mutex, MutexGuard, Once, OnceLock, PoisonError};

/// The jobserver this process inherited, if any (read from `MAKEFLAGS` once).
/// The first call with a live pool also starts the SIGINT/SIGTERM release
/// thread.
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
    if client.pool.is_some() {
        SIGNALS.call_once(|| release_on_signal(client));
    }
    client
}

/// Starts a thread that, on SIGINT or SIGTERM, returns every pool byte the
/// process holds and then ends it with the signal's default action (so the
/// parent still sees "killed by SIGINT"; exit status 128 + signal if that
/// cannot be emulated).
#[cfg(unix)]
fn release_on_signal(client: &'static Client) {
    use signal_hook::consts::{SIGINT, SIGTERM};
    let mut signals = match signal_hook::iterator::Signals::new([SIGINT, SIGTERM]) {
        Ok(s) => s,
        Err(e) => {
            eprintln!(
                "xtask: warning: cannot catch SIGINT/SIGTERM ({e}); an interrupted run \
                 will not return its jobserver tokens"
            );
            return;
        }
    };
    let spawned = std::thread::Builder::new().name("jobserver-signals".into()).spawn(move || {
        if let Some(sig) = signals.forever().next() {
            let n = client.release_all();
            if n > 0 {
                eprintln!("xtask: signal {sig}: returned {n} jobserver token(s)");
            }
            let _ = signal_hook::low_level::emulate_default_handler(sig);
            std::process::exit(128 + sig);
        }
    });
    if let Err(e) = spawned {
        eprintln!("xtask: warning: cannot start the jobserver signal thread ({e})");
    }
}

#[cfg(not(unix))]
fn release_on_signal(_: &'static Client) {}

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
            if st.implicit_free {
                st.implicit_free = false;
                st.waiters -= 1;
                // The read requested for us is no longer needed.
                st.requested = st.requested.saturating_sub(1);
                return token(Kind::Implicit);
            }
            if let Some(b) = st.spare.pop() {
                st.waiters -= 1;
                st.held.push(b);
                return token(Kind::Pool(b));
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

/// Reads one pool byte per request; a byte nobody waits for any more goes
/// straight back to the pool.
fn helper(pool: &Pool) {
    loop {
        {
            let mut st = pool.lock();
            while st.requested == 0 {
                st = pool.helper.wait(st).unwrap_or_else(PoisonError::into_inner);
            }
        }
        let mut byte = [0u8; 1];
        let r = (&pool.fifo).read_exact(&mut byte);
        let mut st = pool.lock();
        st.requested = st.requested.saturating_sub(1);
        match r {
            Ok(()) if st.waiters > st.spare.len() && !st.released => {
                st.spare.push(byte[0]);
                pool.workers.notify_all();
            }
            Ok(()) => {
                drop(st);
                pool.give_back(byte[0]);
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
