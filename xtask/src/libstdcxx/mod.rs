//! `cargo xtask libstdcxx` — per-header status of GNU libstdc++ under kiln:
//! every public header the installed libstdc++ ships is compiled alone
//! (`#include <h>`, `-fsyntax-only`) with kiln and with clang++-19 as the
//! reference, and the outcomes, error counts, first errors, wall time and
//! peak RSS go to a Markdown status page. Contract: `spec/libstdcxx.md`.

mod report;
mod run;

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::util::process::{self, MemLimit, RssProbe};
use crate::util::{Profile, build_kiln_with, repo_root, target_dir};
use run::{Config, HeaderResult};

const USAGE: &str = "\
usage: cargo xtask libstdcxx [options]

Compiles every public libstdc++ header alone (`#include <h>`, -fsyntax-only)
with kiln and with clang++-19 as the reference, and reports per-header status.
Informational: the exit status is non-zero only when the tool itself fails.
Contract: spec/libstdcxx.md.

  --std <std>            c++26 (default), c++23, c++20 or c++17
  --headers <dir>        the libstdc++ include directory to enumerate (default:
                         the newest /usr/include/c++/<version>)
  --profile <p>          the cargo profile to build kiln++ with: debug, harness
                         (default) or release (spec/libcxx.md \"Build profiles\")
  --compiler <path>      kiln++ binary (default: cargo build --profile <p> -p kiln_driver)
  --clang <path>         the reference compiler (default clang++-19)
  --no-clang             skip the reference compiler
  --filter <glob>        only headers whose name matches <glob> (`*`, `?`)
  --list                 list the selected headers and exit
  -j, --jobs <n>         headers in parallel (default 2; an upper bound under a
                         MAKEFLAGS jobserver, see spec/harness.md)
  --timeout <secs>       per-compile timeout (default 120; --profile debug: 300)
  --mem-limit <MiB>      address-space cap per compile (default 8192; 0: none)
  --report <file.md>     write this standard's generated region of the page
  -v, --verbose          print every header's result
";

/// The standards `--std` accepts.
pub const STDS: &[&str] = &["c++26", "c++23", "c++20", "c++17"];

/// Where Debian installs libstdc++'s headers, one directory per GCC version.
pub const SYSTEM_CXX_INCLUDE: &str = "/usr/include/c++";

/// Subdirectories of the include directory that hold no public header
/// (implementation detail, extensions, TS and parallel-mode headers). The
/// enumeration only takes files directly under the directory, so these are
/// never walked; the list documents the skip and is checked by a test.
pub const SKIPPED_DIRS: &[&str] = &[
    "backward",
    "bits",
    "debug",
    "decimal",
    "experimental",
    "ext",
    "parallel",
    "pstl",
    "tr1",
    "tr2",
];

#[derive(Clone, Debug, PartialEq)]
#[allow(clippy::struct_excessive_bools)] // independent command-line switches
pub struct Options {
    pub std: String,
    pub headers: Option<PathBuf>,
    pub compiler: Option<PathBuf>,
    pub profile: Profile,
    pub clang: PathBuf,
    pub no_clang: bool,
    pub filter: Option<String>,
    pub list: bool,
    pub jobs: usize,
    /// `--timeout`; `None` is [`Profile::compile_timeout`].
    pub timeout: Option<Duration>,
    pub mem_limit_mib: u64,
    pub report: Option<PathBuf>,
    pub verbose: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            std: "c++26".to_owned(),
            headers: None,
            compiler: None,
            profile: Profile::Harness,
            clang: PathBuf::from("clang++-19"),
            no_clang: false,
            filter: None,
            list: false,
            jobs: 2,
            timeout: None,
            mem_limit_mib: process::DEFAULT_MEM_LIMIT_MIB,
            report: None,
            verbose: false,
        }
    }
}

impl Options {
    /// The per-compile limit: `--timeout`, else the profile's.
    pub fn timeout(&self) -> Duration {
        self.timeout.unwrap_or_else(|| self.profile.compile_timeout())
    }
}

/// `Ok(None)` means `--help`.
pub fn parse_args(args: impl IntoIterator<Item = String>) -> Result<Option<Options>, String> {
    let mut o = Options::default();
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
            "--std" => {
                let s = value("--std")?;
                if !STDS.contains(&s.as_str()) {
                    return Err(format!("unknown --std `{s}` (known: {})", STDS.join(", ")));
                }
                o.std = s;
            }
            "--headers" => o.headers = Some(PathBuf::from(value("--headers")?)),
            "--compiler" => o.compiler = Some(PathBuf::from(value("--compiler")?)),
            "--profile" => o.profile = Profile::parse(&value("--profile")?)?,
            "--clang" => o.clang = PathBuf::from(value("--clang")?),
            "--filter" => o.filter = Some(value("--filter")?),
            "--report" => o.report = Some(PathBuf::from(value("--report")?)),
            "-j" | "--jobs" => {
                let v = value("--jobs")?;
                o.jobs =
                    v.parse().ok().filter(|&n| n > 0).ok_or_else(|| format!("bad --jobs `{v}`"))?;
            }
            "--timeout" => {
                let v = value("--timeout")?;
                let secs: f64 = v
                    .parse()
                    .ok()
                    .filter(|s: &f64| s.is_finite() && *s > 0.0)
                    .ok_or_else(|| format!("bad --timeout `{v}`"))?;
                o.timeout = Some(Duration::from_secs_f64(secs));
            }
            "--mem-limit" => {
                let v = value("--mem-limit")?;
                o.mem_limit_mib =
                    v.parse().map_err(|_| format!("bad --mem-limit `{v}` (MiB; 0 disables)"))?;
            }
            "--no-clang" | "--list" | "-v" | "--verbose" if inline.is_some() => {
                return Err(format!("`{flag}` takes no value"));
            }
            "--no-clang" => o.no_clang = true,
            "--list" => o.list = true,
            "-v" | "--verbose" => o.verbose = true,
            f => return Err(format!("unknown argument `{f}`")),
        }
    }
    Ok(Some(o))
}

pub fn main(args: impl IntoIterator<Item = String>) -> ExitCode {
    let opts = match parse_args(args) {
        Ok(Some(o)) => o,
        Ok(None) => {
            print!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        Err(e) => {
            eprintln!("xtask libstdcxx: {e}\n\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    match execute(&opts) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("xtask libstdcxx: {e}");
            ExitCode::from(2)
        }
    }
}

/// The newest `<base>/<version>` directory (version: digits and dots,
/// compared numerically), the layout Debian's `libstdc++-N-dev` installs.
pub fn default_headers_dir(base: &Path) -> Option<PathBuf> {
    let key = |name: &str| -> Option<Vec<u64>> {
        name.split('.').map(|p| p.parse().ok()).collect::<Option<Vec<u64>>>()
    };
    std::fs::read_dir(base)
        .ok()?
        .filter_map(Result::ok)
        .filter(|e| e.path().is_dir())
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            key(&name).map(|k| (k, e.path()))
        })
        .max_by(|a, b| a.0.cmp(&b.0))
        .map(|(_, p)| p)
}

/// The public headers of a libstdc++ include directory: the regular files
/// (or links to them) directly under `dir` whose name has no extension,
/// sorted. Subdirectories ([`SKIPPED_DIRS`]) and `.h`/`.tcc` files are not
/// public C++ headers; the `<cxxx>` C compatibility headers have no
/// extension and are included.
pub fn public_headers(dir: &Path) -> Result<Vec<String>, String> {
    let rd = std::fs::read_dir(dir).map_err(|e| format!("cannot read {}: {e}", dir.display()))?;
    let mut v: Vec<String> = rd
        .filter_map(Result::ok)
        .filter(|e| e.path().is_file())
        .filter_map(|e| e.file_name().to_str().map(str::to_owned))
        .filter(|n| !n.contains('.') && !n.starts_with('.'))
        .collect();
    v.sort();
    Ok(v)
}

/// Shell-style glob over a whole name: `*` matches any run, `?` one
/// character, everything else itself.
pub fn glob_match(pattern: &str, name: &str) -> bool {
    let p: Vec<char> = pattern.chars().collect();
    let n: Vec<char> = name.chars().collect();
    // Iterative matcher with single-star backtracking.
    let (mut pi, mut ni) = (0, 0);
    let mut star: Option<(usize, usize)> = None;
    while ni < n.len() {
        if pi < p.len() && (p[pi] == '?' || p[pi] == n[ni]) {
            pi += 1;
            ni += 1;
        } else if pi < p.len() && p[pi] == '*' {
            star = Some((pi, ni));
            pi += 1;
        } else if let Some((sp, sn)) = star {
            pi = sp + 1;
            ni = sn + 1;
            star = Some((sp, sn + 1));
        } else {
            return false;
        }
    }
    p[pi..].iter().all(|&c| c == '*')
}

fn execute(opts: &Options) -> Result<(), String> {
    let start = Instant::now();
    let root = repo_root();
    let headers_dir = match &opts.headers {
        Some(d) => d.clone(),
        None => default_headers_dir(Path::new(SYSTEM_CXX_INCLUDE)).ok_or_else(|| {
            format!("no libstdc++ headers under {SYSTEM_CXX_INCLUDE}; pass --headers <dir>")
        })?,
    };
    let mut headers = public_headers(&headers_dir)?;
    if let Some(f) = &opts.filter {
        headers.retain(|h| glob_match(f, h));
    }
    if headers.is_empty() {
        return Err(format!("no headers selected in {}", headers_dir.display()));
    }
    if opts.list {
        for h in &headers {
            println!("<{h}>");
        }
        println!("{} headers in {} ({})", headers.len(), headers_dir.display(), selection_note());
        return Ok(());
    }

    let target = target_dir(&root)?;
    let kiln = match &opts.compiler {
        Some(c) => c.clone(),
        None => build_kiln_with(&root, &target, "libstdcxx", opts.profile)?,
    };
    let clang = if opts.no_clang {
        None
    } else {
        match crate::util::identify(&opts.clang) {
            Ok(_) => Some(opts.clang.clone()),
            Err(e) => {
                eprintln!(
                    "xtask libstdcxx: warning: {} not runnable ({e}); no reference column",
                    opts.clang.display()
                );
                None
            }
        }
    };
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.subsec_nanos());
    let scratch =
        target.join("xtask-libstdcxx").join(format!("run-{}-{nanos}", std::process::id()));
    std::fs::create_dir_all(&scratch)
        .map_err(|e| format!("cannot create {}: {e}", scratch.display()))?;
    let cfg = Config {
        kiln: kiln.clone(),
        clang,
        resource_dir: root.join("resource"),
        headers_dir: headers_dir.clone(),
        std: opts.std.clone(),
        timeout: opts.timeout(),
        mem: MemLimit::from_mib(opts.mem_limit_mib),
        probe: RssProbe::detect(&scratch),
        scratch: scratch.clone(),
    };
    println!(
        "xtask libstdcxx: {} headers from {} with {}{} at -std={} (-j {}{})",
        headers.len(),
        headers_dir.display(),
        kiln.display(),
        cfg.clang.as_ref().map_or_else(String::new, |c| format!(" and {}", c.display())),
        opts.std,
        opts.jobs,
        crate::jobs::global().describe().map_or_else(String::new, |d| format!(", jobserver {d}")),
    );
    let results = run_all(&cfg, &headers, opts.jobs, opts.verbose);
    let wall = start.elapsed();
    let info = run_info(opts, &cfg, &root, wall);
    let _ = std::fs::remove_dir_all(&scratch);

    let summary = report::Summary::of(&results);
    println!("\n{}", summary.line(&opts.std));
    let clusters = report::clusters_of(&results);
    println!("\nTop first-error clusters:");
    for (k, c) in clusters.ranked().into_iter().take(10) {
        println!("  {:>3} first / {:>3} any  {k}", c.first, c.any);
    }
    if let Some(path) = &opts.report {
        let region = report::render(&info, &results, 30);
        let existing = std::fs::read_to_string(path).unwrap_or_default();
        std::fs::write(path, report::splice(&existing, &opts.std, &region))
            .map_err(|e| format!("cannot write {}: {e}", path.display()))?;
        println!("xtask libstdcxx: wrote {}", path.display());
    }
    // A compiler that could not be started at all is a tool error.
    if let Some(r) = results.iter().find(|r| r.tool_error().is_some()) {
        return Err(format!("<{}>: {}", r.header, r.tool_error().unwrap_or_default()));
    }
    Ok(())
}

fn run_all(cfg: &Config, headers: &[String], jobs: usize, verbose: bool) -> Vec<HeaderResult> {
    let next = AtomicUsize::new(0);
    let done = AtomicUsize::new(0);
    let slots: Mutex<Vec<Option<HeaderResult>>> = Mutex::new(vec![None; headers.len()]);
    std::thread::scope(|s| {
        for _ in 0..jobs.max(1) {
            s.spawn(|| {
                loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    let Some(h) = headers.get(i) else { break };
                    let r = run::run_header(cfg, h, i);
                    let finished = done.fetch_add(1, Ordering::Relaxed) + 1;
                    if verbose || !r.kiln.status.is_ok() {
                        println!("[{finished}/{}] {}", headers.len(), r.console_line());
                    }
                    slots.lock().expect("no worker panics while holding the lock")[i] = Some(r);
                }
            });
        }
    });
    slots
        .into_inner()
        .expect("workers have finished")
        .into_iter()
        .map(|r| r.expect("every header produced a result"))
        .collect()
}

/// What the enumeration leaves out, for `--list` and the page.
pub fn selection_note() -> String {
    let dirs: Vec<String> = SKIPPED_DIRS.iter().map(|d| format!("`{d}/`")).collect();
    format!(
        "extension-less files directly in the include directory; not compiled: subdirectories ({}) and `.h`/`.tcc` files",
        dirs.join(", ")
    )
}

fn run_info(opts: &Options, cfg: &Config, root: &Path, wall: Duration) -> report::RunInfo {
    let commit = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map_or_else(
            || "unknown".to_owned(),
            |o| String::from_utf8_lossy(&o.stdout).trim().to_owned(),
        );
    let version =
        |c: &Path| crate::util::identify(c).map_or_else(|_| "unknown".to_owned(), |id| id.version);
    let mut command = format!("cargo xtask libstdcxx --std {} -j {}", opts.std, opts.jobs);
    if let Some(f) = &opts.filter {
        let _ = write!(command, " --filter '{f}'");
    }
    if cfg.clang.is_none() {
        command.push_str(" --no-clang");
    }
    match &opts.compiler {
        Some(_) => command.push_str(" --compiler <kiln++>"),
        None => {
            let _ = write!(command, " --profile {}", opts.profile.label());
        }
    }
    report::RunInfo {
        std: opts.std.clone(),
        kiln_version: version(&cfg.kiln),
        kiln_commit: commit,
        clang_version: cfg.clang.as_deref().map_or_else(|| "not run".to_owned(), version),
        libstdcxx: run::libstdcxx_version(cfg),
        headers_dir: cfg.headers_dir.display().to_string(),
        selection: selection_note(),
        command,
        flags: cfg.common_flags().join(" "),
        limits: format!(
            "{} per compile (kiln: plus its stack reservation); timeout {} s; -j {}",
            cfg.mem.map_or_else(
                || "no memory cap".to_owned(),
                |m| format!("{} MiB address space", m.mib)
            ),
            cfg.timeout.as_secs_f64(),
            opts.jobs
        ),
        rss_source: cfg.probe.source_id().to_owned(),
        wall,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Option<Options>, String> {
        parse_args(args.iter().map(|s| (*s).to_owned()))
    }

    fn fixture() -> PathBuf {
        repo_root().join("xtask").join("fixtures").join("libstdcxx")
    }

    #[test]
    fn options() {
        let o =
            parse(&["--std=c++20", "-j", "3", "--filter", "c*", "--report", "p.md", "--no-clang"])
                .unwrap()
                .unwrap();
        assert_eq!(o.std, "c++20");
        assert_eq!(o.jobs, 3);
        assert_eq!(o.filter.as_deref(), Some("c*"));
        assert_eq!(o.report, Some(PathBuf::from("p.md")));
        assert!(o.no_clang);
        assert_eq!(parse(&["--help"]), Ok(None));
        assert!(parse(&["--std", "c++98"]).is_err());
        assert!(parse(&["-j", "0"]).is_err());
        assert!(parse(&["--list=1"]).is_err());
        assert!(parse(&["vector"]).is_err());
        let d = Options::default();
        assert_eq!((d.std.as_str(), d.jobs, d.mem_limit_mib), ("c++26", 2, 8192));
        assert_eq!(d.timeout(), Duration::from_secs(120));
        // spec/libcxx.md "Build profiles": the profile sets the default limit.
        let dbg = parse(&["--profile", "debug"]).unwrap().unwrap();
        assert_eq!((dbg.profile, dbg.timeout()), (Profile::Debug, Duration::from_secs(300)));
        let t = parse(&["--profile", "debug", "--timeout", "9"]).unwrap().unwrap();
        assert_eq!(t.timeout(), Duration::from_secs(9));
        assert!(parse(&["--profile", "fast"]).is_err());
    }

    #[test]
    fn enumeration() {
        let dir = fixture().join("include").join("c++").join("14");
        let h = public_headers(&dir).unwrap();
        // Extension-less files only: `.h`, `.tcc` and every subdirectory
        // (bits/, ext/, backward/, ...) are left out; `<cxxx>` C headers stay.
        assert_eq!(h, ["algorithm", "cstdio", "cstdlib", "vector"]);
        for d in ["bits", "ext", "backward", "tr1"] {
            assert!(dir.join(d).is_dir(), "fixture has {d}/");
            assert!(SKIPPED_DIRS.contains(&d));
        }
        assert!(public_headers(&dir.join("missing")).is_err());
        // The newest numeric version directory; `v1` (libc++) is ignored.
        let base = fixture().join("include").join("c++");
        assert_eq!(default_headers_dir(&base), Some(base.join("14")));
        assert_eq!(default_headers_dir(&base.join("missing")), None);
    }

    #[test]
    fn globs() {
        assert!(glob_match("*", "vector"));
        assert!(glob_match("c*", "cstdio"));
        assert!(!glob_match("c*", "vector"));
        assert!(glob_match("c?t*", "cstdlib") && !glob_match("c?t*", "cmath"));
        assert!(glob_match("*map", "unordered_map") && !glob_match("*map", "mapx"));
        assert!(glob_match("vector", "vector") && !glob_match("vector", "vectors"));
        assert!(glob_match("*r*r*", "forward_list") && !glob_match("*z*", "forward_list"));
    }
}
