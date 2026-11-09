//! The toolchain's module-file cache for libc++'s `std` and `std.compat`
//! (spec/modules.md §11.11, spec/driver.md "Module cache").
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
//! writes, and checks only that a hit is a kiln module file.
//!
//! Layout: `<root>/<key>/<name>.pcm` and `<root>/<key>/<name>.stamp`.
//! `<root>` is `-fmodules-cache-path=`, else `$XDG_CACHE_HOME/kiln/modules`,
//! else `$HOME/.cache/kiln/modules`. `<key>` hashes everything that
//! decides the module file's content: the kiln executable's identity, the
//! fingerprint (spec/modules.md §11.4), the predefines (every `-D`, `-U`,
//! `-include` and target macro), the header search list, and the libc++
//! module directory with the text of its `std.cppm` and
//! `std.compat.cppm`. The stamp lists every file the build read with its
//! size and modification time (clang's module-cache validation); a hit
//! needs every one unchanged, so a libc++ header edited in place rebuilds
//! the module.
//!
//! Concurrency: the module file is written to a temporary file and
//! renamed into place (`bmi::write_file`), then the stamp the same way, so
//! a reader never sees a torn file; a reader takes the stamp first and the
//! module file second, so a stamp it accepts was written after the module
//! file it then reads. Two compiles that miss together both build, both
//! rename, and both succeed; the builds are deterministic, so either file
//! is the right one.

use std::fmt::Write as _;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::{Arc, Mutex, PoisonError};

use kiln_diag::DiagOptions;

use crate::SharedWriter;
use crate::args::{Action, Args, DepArgs, DepHeaders};
use crate::pp_cmd::Job;
use crate::syntax_cmd::SyntaxJob;
use crate::toolchain::{LangStd, Toolchain};

/// The first line of a stamp file.
const STAMP_MAGIC: &str = "kiln-module-cache-stamp 1";

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
        // The stamp first, then the module file it vouches for.
        if stamp_valid(&stamp, &pcm) && kiln_pp::bmi::sniff(&pcm) == kiln_pp::bmi::Sniffed::Kiln {
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
        self.remark(&format!("building '{name}' into '{}'", pcm.display()));
        let files = match self.build(source, &pcm) {
            Ok(files) => files,
            Err(why) => {
                // Leave no empty entry behind (a no-op when another
                // compile has filled it).
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

/// Writes the stamp of `pcm`: the module file itself, then `source` and
/// every other file the build read. Written atomically, after `pcm`.
fn write_stamp(stamp: &Path, pcm: &Path, source: &Path, files: &[String]) -> std::io::Result<()> {
    let bad = || std::io::Error::other("a file the build read cannot be stamped");
    let mut text = format!("{STAMP_MAGIC}\n");
    let mut seen = std::collections::HashSet::new();
    let pcm_line = stamp_line(pcm).ok_or_else(bad)?;
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

/// Whether `stamp` is a stamp whose first entry is `pcm` and whose every
/// file is unchanged.
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
        if first && Path::new(path) != pcm {
            return false;
        }
        first = false;
        if stamp_line(Path::new(path)).as_deref() != Some(line) {
            return false;
        }
    }
    !first
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
        assert_eq!(root_from(&a, env(Some("x"), Some("/h"))), Some("/h/.cache/kiln/modules".into()));
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
        std::fs::write(&pcm, b"KILNBMI\0x").unwrap();
        std::fs::write(&src, b"export module std;").unwrap();
        std::fs::write(&hdr, b"int x;").unwrap();
        let stamp = dir.join("std.stamp");
        write_stamp(&stamp, &pcm, &src, &[hdr.display().to_string()]).unwrap();
        assert!(stamp_valid(&stamp, &pcm));
        assert!(!stamp_valid(&stamp, &dir.join("other.pcm")));
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
