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

use std::fmt::Write as _;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

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
pub const DEFAULT_WAIT: Duration = Duration::from_secs(120);

/// The environment variable that replaces [`DEFAULT_WAIT`], in
/// milliseconds (kiln's tests).
const WAIT_ENV: &str = "KILN_MODULE_CACHE_WAIT_MS";

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
    };
    let cache = Rc::new(cache);
    Some(kiln_pp::StdModuleCache(Rc::new(move |name: &str, source: &Path| {
        cache.lookup(name, source)
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
    /// source is `source`): a valid cached one, or one built now; `None`
    /// to build the unit from its source.
    fn lookup(&self, name: &str, source: &Path) -> Option<PathBuf> {
        let dir = self.entry_dir(source);
        let pcm = dir.join(format!("{name}.pcm"));
        let stamp = dir.join(format!("{name}.stamp"));
        // A hit takes no lock: the stamp first, then the module file it
        // vouches for.
        let lock_path = dir.join(format!("{name}.lock"));
        if entry_valid(&stamp, &pcm) {
            // A lock file here was left by a compile killed while it held
            // it; it is removed when free (never waiting for it).
            if lock_path.exists()
                && let Ok(Some(stale)) = EntryLock::acquire(&lock_path, Duration::ZERO, || {})
            {
                drop(stale);
            }
            self.remark(&format!("using '{}' for '{name}'", pcm.display()));
            return Some(pcm);
        }
        if let Err(e) = std::fs::create_dir_all(&dir) {
            self.remark(&format!(
                "cannot create '{}': {e}; building '{name}' from its source",
                dir.display()
            ));
            return None;
        }
        let lock = match EntryLock::acquire(&lock_path, wait_limit(), || {
            self.remark(&format!("waiting for another compile building '{name}'"));
        }) {
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
        // waited.
        if entry_valid(&stamp, &pcm) {
            self.remark(&format!("using '{}' for '{name}'", pcm.display()));
            return Some(pcm);
        }
        // Only a holder of this lock writes these temporary files, so any
        // there now belongs to a holder that was killed.
        remove_temporaries(&dir, name);
        self.remark(&format!("building '{name}' into '{}'", pcm.display()));
        let files = match self.build(source, &pcm) {
            Ok(files) => files,
            Err(why) => {
                // Leave no empty entry behind (a no-op when the entry
                // holds anything else).
                drop(lock);
                let _ = std::fs::remove_dir(&dir);
                self.remark(&format!("{why}; building '{name}' from its source"));
                return None;
            }
        };
        match write_stamp(&stamp, &pcm, source, &files) {
            Ok(()) => Some(pcm),
            Err(e) => {
                // The module file is fine; without a stamp the next
                // compile rebuilds it.
                self.remark(&format!("cannot write '{}': {e}", stamp.display()));
                Some(pcm)
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
        a.deps = DepArgs { side: Some(DepHeaders::All), ..DepArgs::default() };
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

/// Writes the stamp of `pcm`: the module file itself, then `source` and
/// every other file the build read. Written atomically, after `pcm`.
fn write_stamp(stamp: &Path, pcm: &Path, source: &Path, files: &[String]) -> std::io::Result<()> {
    let bad = || std::io::Error::other("a file the build read cannot be stamped");
    let mut text = format!("{STAMP_MAGIC}\n");
    let mut seen = std::collections::HashSet::new();
    let pcm_line = pcm_line(pcm).ok_or_else(bad)?;
    let _ = writeln!(text, "{pcm_line}");
    for f in std::iter::once(source).chain(files.iter().map(Path::new)) {
        if !seen.insert(f.to_owned()) {
            continue;
        }
        let line = stamp_line(f).ok_or_else(bad)?;
        let _ = writeln!(text, "{line}");
    }
    write_atomically(stamp, text.as_bytes())
}

/// Whether `stamp` is a stamp whose first entry is `pcm`, with the
/// checksum `pcm` has now, and whose every other file is unchanged.
fn stamp_valid(stamp: &Path, pcm: &Path) -> bool {
    let Ok(text) = std::fs::read_to_string(stamp) else { return false };
    let mut lines = text.lines();
    if lines.next() != Some(STAMP_MAGIC) {
        return false;
    }
    let mut first = true;
    for line in lines {
        let mut parts = line.splitn(3, ' ');
        let (Some(_), Some(_), Some(path)) = (parts.next(), parts.next(), parts.next()) else {
            return false;
        };
        let now = if first {
            if Path::new(path) != pcm {
                return false;
            }
            pcm_line(pcm)
        } else {
            stamp_line(Path::new(path))
        };
        first = false;
        if now.as_deref() != Some(line) {
            return false;
        }
    }
    !first
}

/// Whether the entry `stamp` + `pcm` is a hit: a valid stamp, read
/// first, and a module file it vouches for that is a kiln module file.
fn entry_valid(stamp: &Path, pcm: &Path) -> bool {
    stamp_valid(stamp, pcm) && kiln_pp::bmi::sniff(pcm) == kiln_pp::bmi::Sniffed::Kiln
}

/// How long a miss waits for the entry's lock: `KILN_MODULE_CACHE_WAIT_MS`
/// when it is a number, else [`DEFAULT_WAIT`].
fn wait_limit() -> Duration {
    std::env::var(WAIT_ENV)
        .ok()
        .and_then(|v| v.trim().parse().ok())
        .map_or(DEFAULT_WAIT, Duration::from_millis)
}

/// Removes the temporary files of `name`'s module file and stamp in
/// `dir` (`<name>.pcm.tmp.*`, `<name>.stamp.tmp.*`): called by the holder
/// of `name`'s lock, the only compile that writes them, so any found is
/// left by a holder that was killed.
fn remove_temporaries(dir: &Path, name: &str) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let prefixes = [format!("{name}.pcm.tmp."), format!("{name}.stamp.tmp.")];
    for e in rd.flatten() {
        let file_name = e.file_name();
        let Some(n) = file_name.to_str() else { continue };
        if prefixes.iter().any(|p| n.starts_with(p.as_str())) {
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
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.subsec_nanos());
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
        write_stamp(&stamp, &pcm, &src, &[hdr.display().to_string()]).unwrap();
        assert!(stamp_valid(&stamp, &pcm));
        assert!(!stamp_valid(&stamp, &dir.join("other.pcm")));
        // The same bytes renamed into place by another compile (a new
        // modification time) still match; other bytes do not.
        std::fs::write(dir.join("std.pcm.new"), b"KILNBMI\0xxxxxxxx12345678").unwrap();
        std::fs::rename(dir.join("std.pcm.new"), &pcm).unwrap();
        assert!(stamp_valid(&stamp, &pcm));
        std::fs::write(&pcm, b"KILNBMI\0xxxxxxxx12345679").unwrap();
        assert!(!stamp_valid(&stamp, &pcm));
        std::fs::write(&pcm, b"KILNBMI\0xxxxxxxx12345678").unwrap();
        assert!(stamp_valid(&stamp, &pcm));
        // A changed size invalidates it.
        std::fs::write(&hdr, b"int xy;").unwrap();
        assert!(!stamp_valid(&stamp, &pcm));
        write_stamp(&stamp, &pcm, &src, &[hdr.display().to_string()]).unwrap();
        assert!(stamp_valid(&stamp, &pcm));
        std::fs::remove_file(&hdr).unwrap();
        assert!(!stamp_valid(&stamp, &pcm));
        std::fs::write(&stamp, "garbage\n").unwrap();
        assert!(!stamp_valid(&stamp, &pcm));
        std::fs::write(&stamp, format!("{STAMP_MAGIC}\n")).unwrap();
        assert!(!stamp_valid(&stamp, &pcm));
        for f in [&pcm, &src, &stamp] {
            std::fs::remove_file(f).unwrap();
        }
        std::fs::remove_dir(&dir).unwrap();
    }
}
