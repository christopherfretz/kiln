//! The toolchain's module-file cache for libc++'s `std` and `std.compat`
//! (spec/modules.md §11.12, spec/driver.md "Module cache").
//!
//! When a translation unit imports `std` or `std.compat` and nothing on
//! the command line supplies it, the preprocessor asks this cache
//! ([`kiln_pp::StdModuleCache`]) instead of building the unit from
//! libc++'s `std.cppm` in-process (part A). A hit is a module file
//! written by an earlier compile with the same configuration; a miss runs
//! `--precompile` of the libc++ source in-process, on its own compile
//! thread, stores the result atomically and uses it. Whatever goes wrong
//! (an unwritable cache, a failed build) falls back to part A, which
//! reports any real error in the importer's own context.
//!
//! The cache is format-agnostic: it stores whatever `--precompile`
//! writes, and checks only that a hit is a kiln module file and reads its
//! trailing checksum (spec/modules.md §11.3), which every kind has.
//!
//! Layout: `<root>/<key>/<name>.pcm` and `<root>/<key>/<name>.stamp`.
//! `<root>` is `-fmodules-cache-path=`, else `$XDG_CACHE_HOME/kiln/modules`,
//! else `$HOME/.cache/kiln/modules`. `<key>` hashes everything that
//! decides the module file's content: the kiln executable's identity, the
//! fingerprint (spec/modules.md §11.4), the predefines (every `-D`, `-U`,
//! `-include` and target macro), the header search list, and the libc++
//! module directory with the text of its `std.cppm` and
//! `std.compat.cppm`. The stamp names the module file by its size and
//! trailing checksum (its content), then lists every file the build read
//! with its size and modification time (clang's module-cache validation);
//! a hit needs every one unchanged, so a libc++ header edited in place
//! rebuilds the module.
//!
//! Concurrency (WP-1074): a hit takes no lock. It reads the stamp first
//! and the module file second; the stamp vouches for the module file's
//! bytes, and every compile building one key writes the same bytes, so a
//! module file renamed into place by any of them matches any of their
//! stamps. A miss takes the entry's `<name>.lock` (an advisory `flock`,
//! released by the kernel when its holder dies), checks the entry again,
//! and only then builds: one compile builds, the others wait and then use
//! what it published. The holder clears any temporary file a killed
//! holder left, writes the module file and then the stamp (each through a
//! temporary file renamed into place), and removes the lock file before
//! releasing it; a waiter that then holds a removed file opens the path
//! again. A waiter gives up after [`DEFAULT_WAIT`]
//! (`KILN_MODULE_CACHE_WAIT_MS`) and builds the module from its source
//! instead, so a stuck compile delays others but never fails them. Where
//! `flock` does not exclude (some network and virtualised file systems),
//! racers all build, as before the lock, and the checksum keeps the entry
//! they leave valid. Before WP-1074 the stamp named the module file by
//! its modification time, so the last module file and the last stamp
//! renamed into place could come from different racers and the entry was
//! a miss for the next compile.
//!
//! Pruning (WP-1050), as clang prunes its implicit module cache
//! ([`maybe_prune`]): after its first lookup, a compile checks
//! `<root>/modules.timestamp`; once that is older than
//! `-fmodules-prune-interval=` (7 days) it is updated and every other
//! entry whose last use is older than `-fmodules-prune-after=` (31 days)
//! is removed, unless a compile holds one of its locks
//! ([`prune_entry`]). The last use is the newest modification time of the
//! entry's files, and a hit refreshes its stamp's at most once a day
//! ([`note_use`]); clang reads access times instead.

use std::fmt::Write as _;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant, SystemTime};

use kiln_diag::DiagOptions;

use crate::SharedWriter;
use crate::args::{Action, Args, DepArgs, DepHeaders};
use crate::pp_cmd::Job;
use crate::syntax_cmd::SyntaxJob;
use crate::toolchain::{LangStd, Toolchain};

/// The first line of a stamp file.
/// Version 2 (WP-1074) names the module file by its checksum, not its
/// modification time.
const STAMP_MAGIC: &str = "kiln-module-cache-stamp 2";

/// How long a miss waits for another compile building the same module
/// before it builds the module from its source instead.
pub const DEFAULT_WAIT: Duration = Duration::from_mins(2);

/// The environment variable that replaces [`DEFAULT_WAIT`], in
/// milliseconds (kiln's tests).
const WAIT_ENV: &str = "KILN_MODULE_CACHE_WAIT_MS";

/// clang's default `-fmodules-prune-interval=` (seconds): the cache is
/// checked for unused entries at most once a week.
pub const DEFAULT_PRUNE_INTERVAL: u32 = 7 * 24 * 60 * 60;

/// clang's default `-fmodules-prune-after=` (seconds): an entry unused
/// for 31 days is removed.
pub const DEFAULT_PRUNE_AFTER: u32 = 31 * 24 * 60 * 60;

/// The file in the cache root whose modification time is the last
/// pruning (clang's name for it).
const TIMESTAMP: &str = "modules.timestamp";

/// At most how often a hit refreshes its stamp's modification time (the
/// entry's last use), unless `-fmodules-prune-after=` is shorter.
const NOTE_USE_EVERY: Duration = Duration::from_hours(24);

/// The suffix a pruner renames `<name>.stamp` to while it removes the
/// entry (see [`prune_entry`]).
const PRUNING: &str = ".pruning";

/// The cache's root directory for `args` (`-fmodules-cache-path=`, then
/// `$XDG_CACHE_HOME/kiln/modules`, then `$HOME/.cache/kiln/modules`), or
/// `None` when the cache is off (`-fno-kiln-module-cache`, an empty
/// `-fmodules-cache-path=`, or no usable home directory).
pub fn cache_root(args: &Args) -> Option<PathBuf> {
    // Made absolute, as clang does, so `-v` and the stamps name one place.
    root_from(args, |v| std::env::var_os(v)).map(|p| std::path::absolute(&p).unwrap_or(p))
}

fn root_from(args: &Args, env: impl Fn(&str) -> Option<std::ffi::OsString>) -> Option<PathBuf> {
    if args.module_cache == Some(false) {
        return None;
    }
    if let Some(p) = &args.modules_cache_path {
        return (!p.is_empty()).then(|| PathBuf::from(p));
    }
    // The XDG base directory specification: a relative value is ignored.
    let xdg = env("XDG_CACHE_HOME").map(PathBuf::from).filter(|p| p.is_absolute());
    let base = xdg.or_else(|| {
        env("HOME").filter(|h| !h.is_empty()).map(|h| PathBuf::from(h).join(".cache"))
    })?;
    Some(base.join("kiln").join("modules"))
}

/// What a cache lookup needs from the importing compile.
struct StdCache {
    root: PathBuf,
    /// The key's material from the importer's configuration (everything
    /// but the libc++ sources, which are read on the first lookup).
    config: String,
    args: Args,
    toolchain: Toolchain,
    std: LangStd,
    diag_opts: DiagOptions,
    /// `-v`: where the cache says what it did.
    verbose: Option<SharedWriter>,
    /// The pruning rules, or `None` when pruning is off.
    prune: Option<Prune>,
    /// Whether this compile has checked the cache for unused entries
    /// (once per compile, after its first lookup).
    pruned: AtomicBool,
}

/// `-fmodules-prune-interval=` and `-fmodules-prune-after=`, both
/// positive (either 0 turns pruning off, as in clang).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Prune {
    /// How often the cache is checked.
    pub interval: Duration,
    /// How long an entry may go unused.
    pub after: Duration,
}

impl Prune {
    /// The rules of `args` (clang's defaults for a flag not given), or
    /// `None` when either is 0.
    pub fn from_args(args: &Args) -> Option<Prune> {
        let interval = args.modules_prune_interval.unwrap_or(DEFAULT_PRUNE_INTERVAL);
        let after = args.modules_prune_after.unwrap_or(DEFAULT_PRUNE_AFTER);
        (interval > 0 && after > 0).then(|| Prune {
            interval: Duration::from_secs(interval.into()),
            after: Duration::from_secs(after.into()),
        })
    }

    /// How old an entry's last use may get before a hit refreshes it:
    /// a day, or half of `after` when that is shorter, so that an entry
    /// a pruner finds unused was not hit since it was last refreshed.
    fn note_use_every(prune: Option<Prune>) -> Duration {
        prune.map_or(NOTE_USE_EVERY, |p| NOTE_USE_EVERY.min(p.after / 2))
    }
}

/// The cache for one importing compile, or `None` when it is off. `predefines`
/// and `search` are the importer's predefines buffer and header search list
/// (as `-v` prints it); `fingerprint` its §11.4 fingerprint.
pub(crate) fn for_job(
    job: &Job<'_>,
    predefines: &str,
    search: &str,
    fingerprint: &[(String, String)],
) -> Option<kiln_pp::StdModuleCache> {
    let a = job.args;
    if a.scan_deps {
        return None;
    }
    let root = cache_root(a)?;
    let mut config = String::new();
    let exe = std::env::current_exe().ok();
    let meta = exe.as_ref().and_then(|e| std::fs::metadata(e).ok());
    let _ = writeln!(
        config,
        "kiln {} {}\nexe {} {} {}",
        kiln_pp::bmi::KILN_VERSION,
        kiln_pp::bmi::kiln_sha(),
        exe.as_ref().map_or_else(String::new, |e| e.display().to_string()),
        meta.as_ref().map_or(0, std::fs::Metadata::len),
        meta.as_ref().map_or_else(String::new, mtime_text),
    );
    for (k, v) in fingerprint {
        let _ = writeln!(config, "fingerprint {k} {v}");
    }
    let _ = write!(config, "predefines {}\n{predefines}\nsearch\n{search}\n", predefines.len());
    let cache = StdCache {
        root,
        config,
        args: a.clone(),
        toolchain: job.toolchain.clone(),
        std: job.std,
        diag_opts: job.diag_opts.clone(),
        verbose: a.verbose.then(|| job.stderr.clone()),
        prune: Prune::from_args(a),
        pruned: AtomicBool::new(false),
    };
    let cache = Rc::new(cache);
    Some(kiln_pp::StdModuleCache(Rc::new(move |name: &str, source: &Path| {
        cache.lookup(name, source).map(|(path, inputs)| kiln_pp::StdModuleFile { path, inputs })
    })))
}

impl StdCache {
    /// Says `what` under `-v`.
    fn remark(&self, what: &str) {
        if let Some(w) = &self.verbose {
            let mut w = w.clone();
            let _ = writeln!(w, "{}: module cache: {what}", self.args.program);
        }
    }

    /// The directory of this configuration's entries: the key hashed from
    /// the configuration and the libc++ module sources beside `source`.
    fn entry_dir(&self, source: &Path) -> PathBuf {
        let mut h = Fnv128::new();
        h.write(self.config.as_bytes());
        let dir = source.parent().unwrap_or(Path::new(""));
        h.write(dir.as_os_str().as_encoded_bytes());
        for f in ["std.cppm", "std.compat.cppm"] {
            let text = std::fs::read(dir.join(f)).unwrap_or_default();
            h.write(f.as_bytes());
            h.write(&(text.len() as u64).to_le_bytes());
            h.write(&text);
        }
        self.root.join(h.hex())
    }

    /// The module file for `name` (`std` or `std.compat`, whose libc++
    /// source is `source`): a valid cached one, or one built now, with
    /// the files its build read as its stamp lists them (the same on a
    /// hit and on a miss, for the importer's dependency file, WP-1051);
    /// `None` to build the unit from its source.
    /// After the compile's first lookup, hit or miss, the cache is
    /// checked for unused entries ([`maybe_prune`]).
    fn lookup(&self, name: &str, source: &Path) -> Option<(PathBuf, Vec<String>)> {
        let dir = self.entry_dir(source);
        let found = self.lookup_in(&dir, name, source);
        if !self.pruned.swap(true, Ordering::Relaxed)
            && let Some(p) = self.prune
        {
            maybe_prune(&self.root, p, &dir, &|m: &str| self.remark(m));
        }
        found
    }

    /// [`Self::lookup`] in the entry `dir`.
    fn lookup_in(&self, dir: &Path, name: &str, source: &Path) -> Option<(PathBuf, Vec<String>)> {
        let note_every = Prune::note_use_every(self.prune);
        let pcm = dir.join(format!("{name}.pcm"));
        let stamp = dir.join(format!("{name}.stamp"));
        let lock_path = dir.join(format!("{name}.lock"));
        // A hit takes no lock: the stamp first, then the module file it
        // vouches for, then the use noted on the stamp, which also checks
        // that no pruner took the entry meanwhile (else it is a miss).
        if let Some(inputs) = entry_valid(&stamp, &pcm)
            && note_use(&stamp, note_every)
        {
            // A lock file here was left by a compile killed while it held
            // it; it is removed when free (never waiting for it).
            if lock_path.exists()
                && let Ok(Some(stale)) = EntryLock::acquire(&lock_path, Duration::ZERO, || {})
            {
                drop(stale);
            }
            self.remark(&format!("using '{}' for '{name}'", pcm.display()));
            return Some((pcm, inputs));
        }
        let mut acquired = Err(std::io::ErrorKind::NotFound.into());
        // Twice: a pruner may remove the directory just created.
        for _ in 0..2 {
            if let Err(e) = std::fs::create_dir_all(dir) {
                self.remark(&format!(
                    "cannot create '{}': {e}; building '{name}' from its source",
                    dir.display()
                ));
                return None;
            }
            acquired = EntryLock::acquire(&lock_path, wait_limit(), || {
                self.remark(&format!("waiting for another compile building '{name}'"));
            });
            if !matches!(&acquired, Err(e) if e.kind() == std::io::ErrorKind::NotFound) {
                break;
            }
        }
        let lock = match acquired {
            Ok(Some(lock)) => lock,
            Ok(None) => {
                self.remark(&format!(
                    "gave up waiting for '{}'; building '{name}' from its source",
                    lock_path.display()
                ));
                return None;
            }
            Err(e) => {
                self.remark(&format!(
                    "cannot lock '{}': {e}; building '{name}' from its source",
                    lock_path.display()
                ));
                return None;
            }
        };
        // Another compile may have published the entry while this one
        // waited (no pruner holds the lock now, so the use noted is kept).
        if let Some(inputs) = entry_valid(&stamp, &pcm) {
            note_use(&stamp, note_every);
            self.remark(&format!("using '{}' for '{name}'", pcm.display()));
            return Some((pcm, inputs));
        }
        // Only a holder of this lock writes these temporary files, so any
        // there now belongs to a holder that was killed.
        remove_temporaries(dir, name);
        self.remark(&format!("building '{name}' into '{}'", pcm.display()));
        let files = match self.build(source, &pcm) {
            Ok(files) => files,
            Err(why) => {
                // Leave no empty entry behind (a no-op when the entry
                // holds anything else).
                drop(lock);
                let _ = std::fs::remove_dir(dir);
                self.remark(&format!("{why}; building '{name}' from its source"));
                return None;
            }
        };
        let inputs = stamp_inputs(source, &files);
        match write_stamp(&stamp, &pcm, &inputs) {
            Ok(()) => Some((pcm, inputs)),
            Err(e) => {
                // The module file is fine; without a stamp the next
                // compile rebuilds it.
                self.remark(&format!("cannot write '{}': {e}", stamp.display()));
                Some((pcm, inputs))
            }
        }
    }

    /// `--precompile` of `source` into `pcm`, in-process on a compile
    /// thread of its own (the importer's stack size and guard), with its
    /// diagnostics kept; the files it read on success.
    fn build(&self, source: &Path, pcm: &Path) -> Result<Vec<String>, String> {
        let mut a = self.args.clone();
        a.action = Action::Precompile;
        a.precompile = true;
        a.output = Some(pcm.display().to_string());
        a.inputs.clear();
        // Every file read, for the stamp; nothing written but the module.
        // A `std` module file `std.compat` imports from this cache is
        // not stamped (its modification time changes whenever a racing
        // compile renames the same bytes into place); its inputs are.
        a.deps = DepArgs {
            side: Some(DepHeaders::All),
            module_files: Some(false),
            ..DepArgs::default()
        };
        a.module_output = None;
        // `std.compat`'s `import std;` goes through this cache as well.
        a.module_files.clear();
        a.nameless_module_files.clear();
        a.prebuilt_module_paths.clear();
        a.verify = None;
        a.dump_ast = None;
        a.dump_ast_filter = None;
        a.dump_record_layouts = false;
        a.dump_vtable_layouts = false;
        a.dump_cfg = false;
        a.cfg_stats = false;
        a.memory_report = false;
        // The importer's profile and report cover its own translation
        // unit only: a BMI's evaluations belong to its build
        // (spec/driver.md "Constexpr profile").
        a.constexpr.stats = false;
        a.constexpr.profile = None;
        a.constexpr.report = None;
        a.crash_for_testing = None;
        a.verbose = false;
        let source_text = source.display().to_string();
        let log = Log::default();
        let size = a.templ.stack_size.unwrap_or(kiln_sema::stack_guard::DEFAULT_STACK_SIZE);
        let result = std::thread::scope(|s| {
            let spawned = std::thread::Builder::new()
                .name("kiln-module-cache".into())
                .stack_size(size)
                .spawn_scoped(s, || {
                    kiln_sema::stack_guard::StackGuard::install(size);
                    let job = Job {
                        args: &a,
                        toolchain: &self.toolchain,
                        std: self.std,
                        path: &source_text,
                        stdin_text: None,
                        diag_opts: &self.diag_opts,
                        stderr: SharedWriter::new(log.clone()),
                    };
                    let r = SyntaxJob(job).run();
                    let files = r.dep_list.map(|l| l.borrow().files().to_vec());
                    (r.errors, files.unwrap_or_default())
                });
            match spawned {
                Ok(h) => h.join().map_err(|_| "building it crashed".to_owned()),
                Err(e) => Err(format!("cannot create its compile thread: {e}")),
            }
        })?;
        match result {
            (0, files) if pcm.is_file() => Ok(files),
            _ => {
                let text = log.text();
                let first = text.lines().find(|l| l.contains("error")).unwrap_or("").to_owned();
                Err(if first.is_empty() {
                    "building it failed".to_owned()
                } else {
                    format!("building it failed: {first}")
                })
            }
        }
    }
}

/// The diagnostics of a cache build, kept in memory.
#[derive(Clone, Default)]
struct Log(Arc<Mutex<Vec<u8>>>);

impl Log {
    fn text(&self) -> String {
        String::from_utf8_lossy(&self.0.lock().unwrap_or_else(PoisonError::into_inner)).into_owned()
    }
}

impl std::io::Write for Log {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner).extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// A file's modification time as `<seconds>.<nanoseconds>` since the
/// epoch (empty when the platform has none).
fn mtime_text(m: &std::fs::Metadata) -> String {
    m.modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or_else(String::new, |d| format!("{}.{:09}", d.as_secs(), d.subsec_nanos()))
}

/// One stamp line for `path`: `<size> <mtime> <path>`, or `None` when it
/// cannot be read or its path cannot be written on one line.
fn stamp_line(path: &Path) -> Option<String> {
    let text = path.to_str()?;
    if text.contains('\n') {
        return None;
    }
    let m = std::fs::metadata(path).ok()?;
    Some(format!("{} {} {text}", m.len(), mtime_text(&m)))
}

/// The stamp line of the module file `path`: `<size> <checksum> <path>`,
/// where `<checksum>` is the file's trailing checksum (spec/modules.md
/// §11.3) in hex: its content's identity, the same for every compile that
/// builds the same module (the writer is deterministic). So a stamp
/// accepts a module file another racing compile renamed into place after
/// it, as long as the bytes are the same. `None` when it cannot be read,
/// is too short to be a module file, or its path cannot be written on one
/// line.
fn pcm_line(path: &Path) -> Option<String> {
    use std::io::{Read as _, Seek as _, SeekFrom};
    let text = path.to_str()?;
    if text.contains('\n') {
        return None;
    }
    let mut f = std::fs::File::open(path).ok()?;
    let len = f.metadata().ok()?.len();
    if len < 16 {
        return None;
    }
    f.seek(SeekFrom::Start(len - 8)).ok()?;
    let mut sum = [0u8; 8];
    f.read_exact(&mut sum).ok()?;
    Some(format!("{len} {:016x} {text}", u64::from_le_bytes(sum)))
}

/// The files a build of `source` that read `files` stamps, in order:
/// `source`, then every other file once.
fn stamp_inputs(source: &Path, files: &[String]) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    std::iter::once(source.display().to_string())
        .chain(files.iter().cloned())
        .filter(|f| seen.insert(PathBuf::from(f)))
        .collect()
}

/// Writes the stamp of `pcm`: the module file itself, then every file
/// the build read ([`stamp_inputs`]). Written atomically, after `pcm`.
fn write_stamp(stamp: &Path, pcm: &Path, inputs: &[String]) -> std::io::Result<()> {
    let bad = || std::io::Error::other("a file the build read cannot be stamped");
    let mut text = format!("{STAMP_MAGIC}\n");
    let pcm_line = pcm_line(pcm).ok_or_else(bad)?;
    let _ = writeln!(text, "{pcm_line}");
    for f in inputs {
        let line = stamp_line(Path::new(f)).ok_or_else(bad)?;
        let _ = writeln!(text, "{line}");
    }
    write_atomically(stamp, text.as_bytes())
}

/// The files the build read, in the stamp's order, when `stamp` is a
/// stamp whose first entry is `pcm`, with the checksum `pcm` has now,
/// and whose every other file is unchanged; else `None`.
fn stamp_valid(stamp: &Path, pcm: &Path) -> Option<Vec<String>> {
    let text = std::fs::read_to_string(stamp).ok()?;
    let mut lines = text.lines();
    if lines.next() != Some(STAMP_MAGIC) {
        return None;
    }
    let mut first = true;
    let mut inputs = Vec::new();
    for line in lines {
        let mut parts = line.splitn(3, ' ');
        let (Some(_), Some(_), Some(path)) = (parts.next(), parts.next(), parts.next()) else {
            return None;
        };
        let now = if first {
            if Path::new(path) != pcm {
                return None;
            }
            pcm_line(pcm)
        } else {
            inputs.push(path.to_owned());
            stamp_line(Path::new(path))
        };
        first = false;
        if now.as_deref() != Some(line) {
            return None;
        }
    }
    (!first).then_some(inputs)
}

/// The files the build of the entry `stamp` + `pcm` read, when it is a
/// hit: a valid stamp, read first, and a module file it vouches for that
/// is a kiln module file.
fn entry_valid(stamp: &Path, pcm: &Path) -> Option<Vec<String>> {
    stamp_valid(stamp, pcm).filter(|_| kiln_pp::bmi::sniff(pcm) == kiln_pp::bmi::Sniffed::Kiln)
}

/// How long a miss waits for the entry's lock: `KILN_MODULE_CACHE_WAIT_MS`
/// when it is a number, else [`DEFAULT_WAIT`].
fn wait_limit() -> Duration {
    std::env::var(WAIT_ENV)
        .ok()
        .and_then(|v| v.trim().parse().ok())
        .map_or(DEFAULT_WAIT, Duration::from_millis)
}

/// Notes a use of the entry whose stamp is `stamp` (a hit): sets the
/// stamp's modification time to now when it is older than `every` (so a
/// hit writes at most once a day), through a handle opened on it. Only
/// the stamp is touched, never the module file, which importers'
/// dependency files list (WP-1051). `false` when the stamp is gone or
/// another file is at its path now: a pruner took the entry (it renames
/// the stamp before checking the last use, [`prune_entry`]), so this is
/// a miss. A stamp that cannot be touched (a cache another user owns)
/// is still a hit.
fn note_use(stamp: &Path, every: Duration) -> bool {
    let Ok(f) = std::fs::File::open(stamp) else { return false };
    let now = SystemTime::now();
    if let Ok(t) = f.metadata().and_then(|m| m.modified())
        && now.duration_since(t).is_ok_and(|age| age > every)
    {
        let _ = f.set_modified(now);
    }
    same_file(&f, stamp)
}

/// How long ago `t` was (zero for a time in the future, as clang's
/// signed difference never exceeds a limit then).
fn age(now: SystemTime, t: SystemTime) -> Duration {
    now.duration_since(t).unwrap_or(Duration::ZERO)
}

/// Whether `name` is a cache key: 32 lowercase hex digits ([`Fnv128`]).
/// Pruning looks at nothing else in the root, so a root shared with
/// other files (clang's own module cache, say) keeps them.
fn is_key(name: &std::ffi::OsStr) -> bool {
    name.to_str().is_some_and(|n| {
        n.len() == 32 && n.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    })
}

/// clang's module-cache pruning (`maybePruneImpl`, clang/lib/Serialization/
/// ModuleCache.cpp in clang 22; `pruneModuleCache` in
/// clang/lib/Frontend/CompilerInstance.cpp in clang 19), for the entries
/// of `root` other than `current` (this compile's own):
///
/// - `<root>/modules.timestamp` missing: create it and stop (the first
///   pruning is one interval after the cache's first use);
/// - its modification time no older than `p.interval`: stop;
/// - else set it to now, then remove every entry unused for longer than
///   `p.after` ([`prune_entry`]).
///
/// Unlike clang, the compile that updates the timestamp holds an
/// advisory lock on it and checks the time again under the lock, so one
/// compile prunes per interval (clang: "a benign race"), and a timestamp
/// that cannot be updated (a read-only cache) means no pruning rather
/// than pruning on every compile. Nothing here fails the compile.
fn maybe_prune(root: &Path, p: Prune, current: &Path, remark: &dyn Fn(&str)) {
    let ts = root.join(TIMESTAMP);
    let meta = match std::fs::symlink_metadata(&ts) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            let _ = std::fs::OpenOptions::new().write(true).create_new(true).open(&ts);
            return;
        }
        Err(_) => return,
    };
    let now = SystemTime::now();
    let stale = |m: &std::fs::Metadata| m.modified().is_ok_and(|t| age(now, t) > p.interval);
    if !meta.is_file() || !stale(&meta) {
        return;
    }
    let Ok(f) = std::fs::OpenOptions::new().read(true).write(true).open(&ts) else { return };
    if f.try_lock().is_err() {
        return;
    }
    // Another compile may have pruned between the check and the lock.
    if !f.metadata().is_ok_and(|m| m.is_file() && stale(&m)) || f.set_modified(now).is_err() {
        return;
    }
    remark(&format!("pruning '{}'", root.display()));
    let Ok(rd) = std::fs::read_dir(root) else { return };
    for e in rd.flatten() {
        let path = e.path();
        // `symlink_metadata`: a link is never followed out of the root.
        if path == current
            || !is_key(&e.file_name())
            || !std::fs::symlink_metadata(&path).is_ok_and(|m| m.is_dir())
        {
            continue;
        }
        prune_entry(&path, p.after, now, remark);
    }
}

/// The newest modification time of the files in the entry `dir` other
/// than lock files (which a pruner creates itself): its stamps (a hit
/// refreshes its stamp, [`note_use`]), its module files and their
/// temporaries (a build writes them); `None` when there is none. Not the
/// access time clang uses: `noatime` and `relatime` mounts do not keep it,
/// and anything reading the files (a backup, an indexer) would update it.
fn last_use(dir: &Path) -> Option<SystemTime> {
    let rd = std::fs::read_dir(dir).ok()?;
    rd.flatten()
        .filter(|e| Path::new(&e.file_name()).extension() != Some("lock".as_ref()))
        .filter_map(|e| std::fs::symlink_metadata(e.path()).ok())
        .filter(std::fs::Metadata::is_file)
        .filter_map(|m| m.modified().ok())
        .max()
}

/// The module name a file of an entry belongs to: `<name>.pcm`,
/// `<name>.stamp`, `<name>.lock`, or a temporary or pruned form of them.
fn entry_file_name(file: &str) -> Option<&str> {
    for sep in [".pcm.tmp.", ".stamp.tmp."] {
        if let Some(at) = file.find(sep) {
            return Some(&file[..at]);
        }
    }
    let file = file.strip_suffix(PRUNING).unwrap_or(file);
    [".pcm", ".stamp", ".lock"].iter().find_map(|s| file.strip_suffix(s))
}

/// Removes the entry `dir` (every file in it, then the directory) when
/// its [`last_use`] is older than `after`, unless a compile uses it:
///
/// 1. it takes the lock of every module the entry has files for (and of
///    `std` and `std.compat`), never waiting: a lock another compile
///    holds (it builds, or waits to) keeps the entry, as does a file for
///    a module not locked;
/// 2. it renames each `<name>.stamp` to `<name>.stamp.pruning`, so a hit
///    that has not noted its use yet finds no stamp and misses (and waits
///    for these locks), then checks the last use again, which counts a
///    hit that noted its use before the rename (through its handle on
///    the stamp, [`note_use`]); a use found renames the stamps back;
/// 3. it removes the entry's files (never a directory in it, and a link
///    as a link), releases the locks, which removes their files, and
///    removes the directory if it is then empty.
///
/// A removal that fails is said under `-v` and otherwise ignored.
fn prune_entry(dir: &Path, after: Duration, now: SystemTime, remark: &dyn Fn(&str)) {
    let unused = |d: &Path| last_use(d).is_none_or(|t| age(now, t) > after);
    if !unused(dir) {
        return;
    }
    let files = |d: &Path| -> Vec<(String, PathBuf)> {
        std::fs::read_dir(d)
            .map(|rd| {
                rd.flatten()
                    .map(|e| (e.file_name().to_string_lossy().into_owned(), e.path()))
                    .collect()
            })
            .unwrap_or_default()
    };
    let mut names: Vec<String> = vec!["std".into(), "std.compat".into()];
    for (f, _) in files(dir) {
        if let Some(n) = entry_file_name(&f)
            && !names.iter().any(|m| m == n)
        {
            names.push(n.to_owned());
        }
    }
    let mut locks = Vec::new();
    for n in &names {
        match EntryLock::acquire(&dir.join(format!("{n}.lock")), Duration::ZERO, || {}) {
            Ok(Some(l)) => locks.push(l),
            _ => return,
        }
    }
    // A compile that created a lock for another module meanwhile.
    let listed = files(dir);
    if listed.iter().any(|(f, _)| entry_file_name(f).is_none_or(|n| !names.iter().any(|m| m == n)))
    {
        return;
    }
    let stamp = |n: &str| dir.join(format!("{n}.stamp"));
    let pruning = |n: &str| dir.join(format!("{n}.stamp{PRUNING}"));
    let renamed: Vec<&String> =
        names.iter().filter(|n| std::fs::rename(stamp(n), pruning(n)).is_ok()).collect();
    if !unused(dir) {
        for n in renamed {
            let _ = std::fs::rename(pruning(n), stamp(n));
        }
        return;
    }
    let held: Vec<PathBuf> = names.iter().map(|n| dir.join(format!("{n}.lock"))).collect();
    let mut failed = false;
    for (_, path) in files(dir) {
        if held.contains(&path) || std::fs::symlink_metadata(&path).is_ok_and(|m| m.is_dir()) {
            continue;
        }
        if let Err(e) = std::fs::remove_file(&path) {
            failed = true;
            remark(&format!("cannot remove '{}': {e}", path.display()));
        }
    }
    drop(locks);
    match std::fs::remove_dir(dir) {
        Ok(()) => remark(&format!("removed unused '{}'", dir.display())),
        // Removed by another pruner, or a compile about to build there
        // created its lock file once these locks were released.
        Err(e)
            if failed
                || matches!(
                    e.kind(),
                    std::io::ErrorKind::NotFound | std::io::ErrorKind::DirectoryNotEmpty
                ) => {}
        Err(e) => remark(&format!("cannot remove '{}': {e}", dir.display())),
    }
}

/// Removes the temporary files of `name`'s module file and stamp in
/// `dir` (`<name>.pcm.tmp.*`, `<name>.stamp.tmp.*`) and a stamp a pruner
/// renamed (`<name>.stamp.pruning`): called by the holder of `name`'s
/// lock, the only compile that writes them (a pruner holds the lock while
/// its renamed stamp exists), so any found is left by a holder that was
/// killed.
fn remove_temporaries(dir: &Path, name: &str) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let prefixes = [format!("{name}.pcm.tmp."), format!("{name}.stamp.tmp.")];
    let pruning = format!("{name}.stamp{PRUNING}");
    for e in rd.flatten() {
        let file_name = e.file_name();
        let Some(n) = file_name.to_str() else { continue };
        if n == pruning || prefixes.iter().any(|p| n.starts_with(p.as_str())) {
            let _ = std::fs::remove_file(e.path());
        }
    }
}

/// The exclusive lock of one cache entry's module, held while it is
/// built and published: an advisory lock (`flock`) on `<name>.lock` in
/// the entry. The kernel releases it when its holder exits, killed or
/// not. Dropping it removes the lock file, then releases the lock, so a
/// finished entry holds no lock file.
#[derive(Debug)]
struct EntryLock {
    file: std::fs::File,
    path: PathBuf,
}

impl EntryLock {
    /// Locks `path`, creating it, waiting at most `wait` for another
    /// holder; `on_wait` is called once when the lock is held by another.
    /// `Ok(None)` when the wait ran out.
    fn acquire(
        path: &Path,
        wait: Duration,
        on_wait: impl FnOnce(),
    ) -> std::io::Result<Option<EntryLock>> {
        let deadline = Instant::now() + wait;
        let mut on_wait = Some(on_wait);
        loop {
            let file = std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(false)
                .open(path)?;
            let mut delay = Duration::from_millis(1);
            loop {
                match file.try_lock() {
                    Ok(()) => break,
                    Err(std::fs::TryLockError::WouldBlock) => {
                        let now = Instant::now();
                        if now >= deadline {
                            return Ok(None);
                        }
                        if let Some(f) = on_wait.take() {
                            f();
                        }
                        std::thread::sleep(delay.min(deadline - now));
                        delay = (delay * 2).min(Duration::from_millis(50));
                    }
                    Err(std::fs::TryLockError::Error(e)) => return Err(e),
                }
            }
            // The holder before this one removed the file it held before
            // releasing it: this lock is on a removed file, so take the
            // path again.
            if same_file(&file, path) {
                return Ok(Some(EntryLock { file, path: path.to_owned() }));
            }
        }
    }
}

impl Drop for EntryLock {
    fn drop(&mut self) {
        // Removed while still held: a compile that opens the path after
        // this creates a new file, and one that opened this file before
        // finds it removed once it locks it.
        let _ = std::fs::remove_file(&self.path);
        let _ = self.file.unlock();
    }
}

/// Whether the open `file` is the file now at `path`.
#[cfg(unix)]
fn same_file(file: &std::fs::File, path: &Path) -> bool {
    use std::os::unix::fs::MetadataExt as _;
    match (file.metadata(), std::fs::metadata(path)) {
        (Ok(a), Ok(b)) => a.dev() == b.dev() && a.ino() == b.ino(),
        _ => false,
    }
}

/// Whether the open `file` is the file now at `path` (no file identity
/// here; a file still at `path` is taken to be it).
#[cfg(not(unix))]
fn same_file(_file: &std::fs::File, path: &Path) -> bool {
    path.is_file()
}

/// Writes `bytes` to `path` through a temporary file beside it, named
/// for this process, thread and moment, renamed into place.
fn write_atomically(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let nanos =
        SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.subsec_nanos());
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(format!(".tmp.{}.{:?}.{nanos}", std::process::id(), std::thread::current().id()));
    let tmp = PathBuf::from(tmp);
    let res = std::fs::File::create(&tmp)
        .and_then(|mut f| f.write_all(bytes).and_then(|()| f.flush()))
        .and_then(|()| std::fs::rename(&tmp, path));
    if res.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    res
}

/// Two FNV-1a 64 hashes with different offsets, for a 128-bit key.
struct Fnv128(u64, u64);

impl Fnv128 {
    const PRIME: u64 = 0x0000_0100_0000_01b3;

    fn new() -> Fnv128 {
        Fnv128(0xcbf2_9ce4_8422_2325, 0x6c62_272e_07bb_0142)
    }

    fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.0 = (self.0 ^ u64::from(b)).wrapping_mul(Self::PRIME);
            self.1 = (self.1 ^ u64::from(b)).wrapping_mul(Self::PRIME).rotate_left(5);
        }
        // A separator, so that ("ab", "c") and ("a", "bc") differ.
        self.0 = (self.0 ^ 0xff).wrapping_mul(Self::PRIME);
        self.1 = (self.1 ^ 0xff).wrapping_mul(Self::PRIME).rotate_left(5);
    }

    fn hex(&self) -> String {
        format!("{:016x}{:016x}", self.0, self.1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(argv: &[&str]) -> Args {
        let v: Vec<String> = argv.iter().map(|s| (*s).to_owned()).collect();
        Args::parse_as("kiln++", &v)
    }

    #[test]
    fn root_resolution() {
        let env = |xdg: Option<&'static str>, home: Option<&'static str>| {
            move |v: &str| match v {
                "XDG_CACHE_HOME" => xdg.map(Into::into),
                "HOME" => home.map(Into::into),
                _ => None,
            }
        };
        let a = args(&["-c", "x.cpp"]);
        assert_eq!(root_from(&a, env(Some("/x"), Some("/h"))), Some("/x/kiln/modules".into()));
        // A relative XDG_CACHE_HOME is ignored.
        assert_eq!(
            root_from(&a, env(Some("x"), Some("/h"))),
            Some("/h/.cache/kiln/modules".into())
        );
        assert_eq!(root_from(&a, env(None, Some("/h"))), Some("/h/.cache/kiln/modules".into()));
        assert_eq!(root_from(&a, env(None, Some(""))), None);
        assert_eq!(root_from(&a, env(None, None)), None);
        let p = args(&["-fmodules-cache-path=/c", "x.cpp"]);
        assert_eq!(root_from(&p, env(Some("/x"), None)), Some("/c".into()));
        let off = args(&["-fmodules-cache-path=", "x.cpp"]);
        assert_eq!(root_from(&off, env(Some("/x"), None)), None);
        let no = args(&["-fmodules-cache-path=/c", "-fno-kiln-module-cache", "x.cpp"]);
        assert_eq!(root_from(&no, env(Some("/x"), None)), None);
        let back = args(&["-fno-kiln-module-cache", "-fkiln-module-cache", "x.cpp"]);
        assert_eq!(root_from(&back, env(Some("/x"), None)), Some("/x/kiln/modules".into()));
    }

    /// WP-1050: the pruning rules from the flags (clang's defaults; 0
    /// turns pruning off), and how often a hit notes its use.
    #[test]
    fn prune_rules() {
        let day = Duration::from_hours(24);
        let p = Prune::from_args(&args(&["x.cpp"])).unwrap();
        assert_eq!((p.interval, p.after), (7 * day, 31 * day));
        assert_eq!(Prune::note_use_every(Some(p)), day);
        assert_eq!(Prune::note_use_every(None), day);
        let p = Prune::from_args(&args(&["-fmodules-prune-after=600", "x.cpp"])).unwrap();
        assert_eq!(Prune::note_use_every(Some(p)), Duration::from_mins(5));
        assert_eq!(Prune::from_args(&args(&["-fmodules-prune-interval=0", "x.cpp"])), None);
        assert_eq!(Prune::from_args(&args(&["-fmodules-prune-after=0", "x.cpp"])), None);
        assert!(is_key(std::ffi::OsStr::new(&"0123456789abcdef".repeat(2))));
        assert!(!is_key(std::ffi::OsStr::new(&"0123456789ABCDEF".repeat(2))));
        assert!(!is_key(std::ffi::OsStr::new("modules.timestamp")));
        for (f, n) in [
            ("std.pcm", Some("std")),
            ("std.compat.stamp", Some("std.compat")),
            ("std.compat.lock", Some("std.compat")),
            ("std.pcm.tmp.42", Some("std")),
            ("std.compat.stamp.tmp.1.ThreadId(1).2", Some("std.compat")),
            ("std.stamp.pruning", Some("std")),
            ("notes.txt", None),
        ] {
            assert_eq!(entry_file_name(f), n, "{f}");
        }
    }

    /// WP-1050: a hit refreshes its stamp at most once per `every`, and
    /// misses once a pruner has renamed the stamp away.
    #[test]
    fn note_use_refreshes_and_sees_a_pruner() {
        let dir = std::env::temp_dir().join(format!("kiln-note-use-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let stamp = dir.join("std.stamp");
        std::fs::write(&stamp, "s").unwrap();
        let age_of =
            |f: &Path| age(SystemTime::now(), std::fs::metadata(f).unwrap().modified().unwrap());
        let set_age = |f: &Path, secs: u64| {
            let t = SystemTime::now() - Duration::from_secs(secs);
            std::fs::File::options().write(true).open(f).unwrap().set_modified(t).unwrap();
        };
        let day = Duration::from_hours(24);
        set_age(&stamp, 3600);
        assert!(note_use(&stamp, day));
        assert!(age_of(&stamp) >= Duration::from_hours(1), "refreshed at most daily");
        set_age(&stamp, 2 * 86_400);
        assert!(note_use(&stamp, day));
        assert!(age_of(&stamp) < Duration::from_mins(1));
        let pruning = dir.join("std.stamp.pruning");
        std::fs::rename(&stamp, &pruning).unwrap();
        assert!(!note_use(&stamp, day));
        // Restored, or another file in its place: only the first is a hit.
        std::fs::rename(&pruning, &stamp).unwrap();
        assert!(note_use(&stamp, day));
        std::fs::remove_file(&stamp).unwrap();
        std::fs::remove_dir(&dir).unwrap();
    }

    #[test]
    fn key_hash_separates_fields() {
        let mut a = Fnv128::new();
        a.write(b"ab");
        a.write(b"c");
        let mut b = Fnv128::new();
        b.write(b"a");
        b.write(b"bc");
        assert_ne!(a.hex(), b.hex());
        assert_eq!(a.hex().len(), 32);
    }

    #[test]
    fn stamps() {
        let dir = std::env::temp_dir().join(format!("kiln-stamp-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let pcm = dir.join("std.pcm");
        let src = dir.join("std.cppm");
        let hdr = dir.join("h.h");
        std::fs::write(&pcm, b"KILNBMI\0xxxxxxxx12345678").unwrap();
        std::fs::write(&src, b"export module std;").unwrap();
        std::fs::write(&hdr, b"int x;").unwrap();
        let stamp = dir.join("std.stamp");
        write_stamp(&stamp, &pcm, &stamp_inputs(&src, &[hdr.display().to_string()])).unwrap();
        // The inputs come back in the order written (for the depfile).
        let inputs = vec![src.display().to_string(), hdr.display().to_string()];
        assert_eq!(stamp_valid(&stamp, &pcm), Some(inputs.clone()));
        assert_eq!(
            stamp_inputs(&src, &[hdr.display().to_string(), src.display().to_string()]),
            inputs
        );
        assert!(stamp_valid(&stamp, &dir.join("other.pcm")).is_none());
        // The same bytes renamed into place by another compile (a new
        // modification time) still match; other bytes do not.
        std::fs::write(dir.join("std.pcm.new"), b"KILNBMI\0xxxxxxxx12345678").unwrap();
        std::fs::rename(dir.join("std.pcm.new"), &pcm).unwrap();
        assert!(stamp_valid(&stamp, &pcm).is_some());
        std::fs::write(&pcm, b"KILNBMI\0xxxxxxxx12345679").unwrap();
        assert!(stamp_valid(&stamp, &pcm).is_none());
        std::fs::write(&pcm, b"KILNBMI\0xxxxxxxx12345678").unwrap();
        assert!(stamp_valid(&stamp, &pcm).is_some());
        // A changed size invalidates it.
        std::fs::write(&hdr, b"int xy;").unwrap();
        assert!(stamp_valid(&stamp, &pcm).is_none());
        write_stamp(&stamp, &pcm, &stamp_inputs(&src, &[hdr.display().to_string()])).unwrap();
        assert!(stamp_valid(&stamp, &pcm).is_some());
        std::fs::remove_file(&hdr).unwrap();
        assert!(stamp_valid(&stamp, &pcm).is_none());
        std::fs::write(&stamp, "garbage\n").unwrap();
        assert!(stamp_valid(&stamp, &pcm).is_none());
        std::fs::write(&stamp, format!("{STAMP_MAGIC}\n")).unwrap();
        assert!(stamp_valid(&stamp, &pcm).is_none());
        for f in [&pcm, &src, &stamp] {
            std::fs::remove_file(f).unwrap();
        }
        std::fs::remove_dir(&dir).unwrap();
    }
}
