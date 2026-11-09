//! Compiling one header with kiln and the reference compiler
//! (`spec/libstdcxx.md` "Commands" and "Outcomes").

use std::fmt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use crate::libcxx::cluster;
use crate::util::process::{self, Measured, MemLimit, RssProbe};

/// Everything a header run needs that does not vary per header.
pub struct Config {
    pub kiln: PathBuf,
    /// The reference compiler; `None` with `--no-clang` or when it cannot run.
    pub clang: Option<PathBuf>,
    pub resource_dir: PathBuf,
    /// The enumerated include directory (reported, and stripped from paths
    /// in first-error lines).
    pub headers_dir: PathBuf,
    /// `--gcc-toolchain=` for both compilers (`xtask libstdcxx --gcc-toolchain`).
    pub gcc_toolchain: Option<PathBuf>,
    pub std: String,
    pub timeout: Duration,
    pub mem: Option<MemLimit>,
    pub probe: RssProbe,
    /// The per-run scratch root; header `i` uses `<root>/h<i>`.
    pub scratch: PathBuf,
}

impl Config {
    /// The flags both compilers get, before the source file.
    pub fn common_flags(&self) -> Vec<String> {
        let mut v = vec![
            format!("-std={}", self.std),
            "-stdlib=libstdc++".to_owned(),
            "-fsyntax-only".to_owned(),
            "-ferror-limit=0".to_owned(),
        ];
        v.extend(self.gcc_toolchain_flag());
        v
    }

    /// `--gcc-toolchain=<dir>`, if one was given.
    pub fn gcc_toolchain_flag(&self) -> Option<String> {
        self.gcc_toolchain.as_ref().map(|t| format!("--gcc-toolchain={}", t.display()))
    }
}

/// How one compile ended.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Status {
    Ok,
    /// Rejected with diagnostics: the `error:` line count and the first one.
    Fail {
        errors: usize,
        first: String,
    },
    /// A panic, an LLVM crash banner or a signal; the crash cluster.
    Crash(String),
    Timeout,
    /// Ran out of the address-space cap.
    Killed,
    /// The compiler could not be started (a tool error).
    Error(String),
}

impl Status {
    pub fn is_ok(&self) -> bool {
        matches!(self, Self::Ok)
    }

    pub fn tag(&self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Fail { .. } => "fail",
            Self::Crash(_) => "crash",
            Self::Timeout => "timeout",
            Self::Killed => "killed",
            Self::Error(_) => "error",
        }
    }

    /// The `error:` count of a failed compile, else 0.
    pub fn errors(&self) -> usize {
        match self {
            Self::Fail { errors, .. } => *errors,
            _ => 0,
        }
    }
}

impl fmt::Display for Status {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Fail { errors, first } => write!(f, "fail ({errors} errors): {first}"),
            Self::Crash(k) | Self::Error(k) => write!(f, "{}: {k}", self.tag()),
            _ => f.write_str(self.tag()),
        }
    }
}

/// One compile of one header.
#[derive(Clone, Debug)]
pub struct Compile {
    pub status: Status,
    /// Normalized `error:` cluster keys with a raw example each, in order of
    /// first appearance (`spec/libcxx.md` "Clusters", without a stage prefix).
    pub keys: Vec<(String, String)>,
    pub wall: Duration,
    pub max_rss_kb: Option<u64>,
}

/// One header's results.
#[derive(Clone, Debug)]
pub struct HeaderResult {
    /// The name inside the angle brackets.
    pub header: String,
    pub kiln: Compile,
    /// `None` when the reference compiler was not run.
    pub clang: Option<Compile>,
}

impl HeaderResult {
    /// Whether the reference compiler rejected the header (so a kiln failure
    /// is not counted against kiln).
    pub fn clang_rejects(&self) -> bool {
        self.clang.as_ref().is_some_and(|c| !c.status.is_ok())
    }

    pub fn tool_error(&self) -> Option<String> {
        let err = |c: &Compile| match &c.status {
            Status::Error(e) => Some(e.clone()),
            _ => None,
        };
        err(&self.kiln).or_else(|| self.clang.as_ref().and_then(err))
    }

    pub fn console_line(&self) -> String {
        let clang = match &self.clang {
            None => String::new(),
            Some(c) if c.status.is_ok() => String::new(),
            Some(c) => format!(" [clang++-19 rejects it too: {}]", c.status.tag()),
        };
        format!("<{}>: {}{clang}", self.header, self.kiln.status)
    }
}

/// Compiles `#include <header>` with kiln, then the reference compiler.
pub fn run_header(cfg: &Config, header: &str, index: usize) -> HeaderResult {
    let dir = cfg.scratch.join(format!("h{index}"));
    let src = dir.join("t.cpp");
    let prepared = std::fs::create_dir_all(&dir)
        .and_then(|()| std::fs::write(&src, format!("#include <{header}>\n")));
    if let Err(e) = prepared {
        let c = Compile {
            status: Status::Error(format!("cannot write {}: {e}", src.display())),
            keys: Vec::new(),
            wall: Duration::ZERO,
            max_rss_kb: None,
        };
        return HeaderResult { header: header.to_owned(), kiln: c.clone(), clang: Some(c) };
    }
    let mut kiln = Command::new(&cfg.kiln);
    // No crash reproducer per crashing header (spec/libstdcxx.md).
    kiln.arg("-resource-dir")
        .arg(&cfg.resource_dir)
        .arg("-fno-crash-diagnostics")
        .args(cfg.common_flags())
        .arg(&src);
    let kiln_args: Vec<_> = kiln.get_args().map(std::ffi::OsStr::to_os_string).collect();
    let kiln_limit = cfg.mem.map(|m| m.plus(process::kiln_stack_reservation_mib(&kiln_args)));
    let k = compile(cfg, kiln, &dir, kiln_limit);
    let c = cfg.clang.as_ref().map(|clang| {
        let mut cmd = Command::new(clang);
        cmd.args(cfg.common_flags()).arg(&src);
        compile(cfg, cmd, &dir, cfg.mem)
    });
    let _ = std::fs::remove_dir_all(&dir);
    HeaderResult { header: header.to_owned(), kiln: k, clang: c }
}

fn compile(cfg: &Config, mut cmd: Command, dir: &Path, limit: Option<MemLimit>) -> Compile {
    cmd.current_dir(dir).env("TMPDIR", dir);
    match process::run_measured(&cmd, &cfg.probe, &dir.join("rss.txt"), cfg.timeout, limit) {
        Ok(m) => judge(&m, &[cfg.headers_dir.as_path(), dir]),
        Err(e) => Compile {
            status: Status::Error(format!("cannot run {}: {e}", cmd.get_program().display())),
            keys: Vec::new(),
            wall: Duration::ZERO,
            max_rss_kb: None,
        },
    }
}

/// Classifies a finished compile. `strip` are directory prefixes removed
/// from the first-error line (the include directory, the scratch dir).
pub fn judge(m: &Measured, strip: &[&Path]) -> Compile {
    let stderr = String::from_utf8_lossy(&m.stderr);
    let mut c =
        Compile { status: Status::Ok, keys: Vec::new(), wall: m.wall, max_rss_kb: m.max_rss_kb };
    if m.status.is_none() {
        c.status = Status::Timeout;
        return c;
    }
    let out = m.as_output();
    if process::out_of_memory(m.status, &stderr) {
        c.status = Status::Killed;
        return c;
    }
    if out.success() {
        return c;
    }
    c.keys = cluster::error_keys(&stderr);
    let signalled = out.code().is_none_or(|code| code >= 128);
    if let Some((key, _)) = cluster::crash_key(&stderr) {
        c.status = Status::Crash(key.trim_start_matches("crash: ").to_owned());
        return c;
    }
    if signalled {
        c.status = Status::Crash(out.describe_end());
        return c;
    }
    let error_lines: Vec<&str> =
        stderr.lines().filter(|l| cluster::error_message(l).is_some()).collect();
    let first = error_lines.first().map_or_else(
        || stderr.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("").to_owned(),
        |l| shorten(l.trim(), strip),
    );
    c.status = Status::Fail { errors: error_lines.len(), first };
    c
}

/// `line` with each `<prefix>/` of `strip` removed. A location under any
/// `.../c++/<version>/` directory (`<version>` the first prefix's last
/// component; the compilers reach the headers through
/// `<gcc>/../../../../include/c++/14` or the target's
/// `include/<triple>/c++/14`) is cut to the part after it.
fn shorten(line: &str, strip: &[&Path]) -> String {
    let mut s = line.to_owned();
    if let Some(version) = strip.first().and_then(|p| p.file_name()).and_then(|v| v.to_str()) {
        let marker = format!("/c++/{version}/");
        let loc_end = s.find(": ").unwrap_or(s.len());
        if let Some(i) = s[..loc_end].rfind(&marker) {
            s = s[i + marker.len()..].to_owned();
        }
    }
    for p in strip {
        let prefix = format!("{}/", p.display());
        s = s.replace(&prefix, "");
    }
    s
}

/// `_GLIBCXX_RELEASE` and `__GLIBCXX__` as the reference compiler (else
/// kiln) preprocesses `<version>`, for the page header.
pub fn libstdcxx_version(cfg: &Config) -> String {
    let dir = cfg.scratch.join("version");
    let src = dir.join("v.cpp");
    if std::fs::create_dir_all(&dir).is_err()
        || std::fs::write(&src, "#include <version>\n").is_err()
    {
        return "unknown".to_owned();
    }
    let compiler = cfg.clang.as_ref().unwrap_or(&cfg.kiln);
    let mut cmd = Command::new(compiler);
    cmd.args([format!("-std={}", cfg.std).as_str(), "-stdlib=libstdc++", "-E", "-dM"])
        .args(cfg.gcc_toolchain_flag())
        .arg(&src)
        .env("TMPDIR", &dir);
    let out = process::run(&mut process::limited(&cmd, cfg.mem), cfg.timeout);
    let _ = std::fs::remove_dir_all(&dir);
    let text = out.map(|o| o.stdout_str()).unwrap_or_default();
    let get = |name: &str| {
        text.lines().find_map(|l| {
            l.strip_prefix("#define ")?.strip_prefix(name)?.strip_prefix(' ').map(str::trim)
        })
    };
    match (get("_GLIBCXX_RELEASE"), get("__GLIBCXX__")) {
        (Some(r), Some(d)) => format!("`_GLIBCXX_RELEASE` {r}, `__GLIBCXX__` {d}"),
        _ => "unknown".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn judging() {
        use std::os::unix::process::ExitStatusExt;
        use std::process::ExitStatus;
        let m = |raw: i32, stderr: &str| Measured {
            status: Some(ExitStatus::from_raw(raw)),
            stderr: stderr.as_bytes().to_vec(),
            wall: Duration::from_millis(5),
            max_rss_kb: Some(100),
        };
        let inc = Path::new("/usr/include/c++/14");
        let st = |raw, s: &str| judge(&m(raw, s), &[inc]).status;
        assert_eq!(st(0, ""), Status::Ok);
        let two = "/usr/include/c++/14/bits/x.h:3:1: error: unknown type name '__typeof'\n\
                   note: here\n/usr/include/c++/14/y:9:2: error: expected ';'\n2 errors generated.\n";
        assert_eq!(
            st(256, two),
            Status::Fail {
                errors: 2,
                first: "bits/x.h:3:1: error: unknown type name '__typeof'".into()
            }
        );
        let c = judge(&m(256, two), &[inc]);
        assert_eq!(c.keys[0].0, "unknown type name '…'");
        assert_eq!((c.wall, c.max_rss_kb), (Duration::from_millis(5), Some(100)));
        let panic =
            "a:1:1: error: x\nthread 'kiln' panicked at crates/kiln_sema/src/a.rs:1:2:\nboom 3\n";
        assert_eq!(
            st(101 << 8, panic),
            Status::Crash("panicked at crates/kiln_sema/src/a.rs: boom N".into())
        );
        assert_eq!(st(11, ""), Status::Crash("killed by signal 11".into()));
        assert_eq!(st(134, "memory allocation of 8 bytes failed\n"), Status::Killed);
        // No `error:` line: the first stderr line stands in.
        assert_eq!(
            st(256, "\nsomething odd\n"),
            Status::Fail { errors: 0, first: "something odd".into() }
        );
        // Paths through the GCC installation and the triple directory.
        let gcc = "/usr/lib/gcc/aarch64-linux-gnu/14/../../../../include/c++/14/type_traits:788:3: error: expected unqualified-id\n";
        assert_eq!(
            st(256, gcc),
            Status::Fail {
                errors: 1,
                first: "type_traits:788:3: error: expected unqualified-id".into()
            }
        );
        let triple =
            "/usr/include/aarch64-linux-gnu/c++/14/bits/c++locale.h:52:14: error: x in /c++/14/y\n";
        assert_eq!(
            st(256, triple),
            Status::Fail {
                errors: 1,
                first: "bits/c++locale.h:52:14: error: x in /c++/14/y".into()
            }
        );
        let timed_out =
            Measured { status: None, stderr: Vec::new(), wall: Duration::ZERO, max_rss_kb: None };
        assert_eq!(judge(&timed_out, &[]).status, Status::Timeout);
        assert_eq!(Status::Fail { errors: 3, first: "e".into() }.errors(), 3);
        assert_eq!(Status::Timeout.errors(), 0);
    }
}
