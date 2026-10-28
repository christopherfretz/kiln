//! `cargo xtask test`: the conformance/differential test runner.
//!
//! The contract (CLI, directive semantics, exit codes, output) is
//! `spec/harness.md`; the directive format is `docs/VALIDATION.md`.

mod compare;
mod diag_parse;
mod directives;
mod discover;
mod process;
mod report;
mod runner;

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use directives::CompilerKind;
use runner::TestResult;

pub const USAGE: &str = "\
usage: cargo xtask test [options] [paths...]

Runs tests/conformance and tests/differential (or the given files/directories)
through a compiler. See spec/harness.md.

options:
  --compiler <path>   compiler under test (default: build and use the kiln binary)
  --oracle <path>     oracle compiler for run/pp comparisons (default: clang++-19)
  --no-oracle         skip all oracle comparisons
  --filter <substr>   only tests whose path contains <substr>
  --update            rewrite existing pp `.expected` files with the actual output
  -j, --jobs <N>      parallel tests (default: min(available CPUs, 8))
  --timeout <secs>    run-time limit for test binaries (default: 10)
  -v, --verbose       print every result and the commands that were run
  -h, --help          this text

exit status: 0 all passed (xfail/skip allowed), 1 a test failed/xpassed/errored,
2 usage or infrastructure error.
";

/// Parsed command line.
#[derive(Clone, Debug, PartialEq)]
pub struct Options {
    pub compiler: Option<PathBuf>,
    pub oracle: PathBuf,
    pub no_oracle: bool,
    pub filter: Option<String>,
    pub update: bool,
    pub jobs: Option<usize>,
    pub run_timeout: Duration,
    pub verbose: bool,
    pub paths: Vec<PathBuf>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            compiler: None,
            oracle: PathBuf::from("clang++-19"),
            no_oracle: false,
            filter: None,
            update: false,
            jobs: None,
            run_timeout: Duration::from_secs(10),
            verbose: false,
            paths: Vec::new(),
        }
    }
}

/// `Ok(None)` means `--help` was requested.
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
            "--compiler" => o.compiler = Some(PathBuf::from(value("--compiler")?)),
            "--oracle" => o.oracle = PathBuf::from(value("--oracle")?),
            "--filter" => o.filter = Some(value("--filter")?),
            "-j" | "--jobs" => {
                let v = value("--jobs")?;
                o.jobs = Some(
                    v.parse().ok().filter(|&n| n > 0).ok_or_else(|| format!("bad --jobs `{v}`"))?,
                );
            }
            "--timeout" => {
                let v = value("--timeout")?;
                let secs: f64 = v
                    .parse()
                    .ok()
                    .filter(|s: &f64| s.is_finite() && *s > 0.0)
                    .ok_or_else(|| format!("bad --timeout `{v}`"))?;
                o.run_timeout = Duration::from_secs_f64(secs);
            }
            "--no-oracle" | "--update" | "-v" | "--verbose" if inline.is_some() => {
                return Err(format!("`{flag}` takes no value"));
            }
            "--no-oracle" => o.no_oracle = true,
            "--update" => o.update = true,
            "-v" | "--verbose" => o.verbose = true,
            f if f.starts_with('-') => return Err(format!("unknown option `{f}`")),
            _ => o.paths.push(PathBuf::from(arg)),
        }
    }
    Ok(Some(o))
}

/// Entry point for `cargo xtask test`.
pub fn main(args: impl IntoIterator<Item = String>) -> ExitCode {
    let opts = match parse_args(args) {
        Ok(Some(o)) => o,
        Ok(None) => {
            print!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        Err(e) => {
            eprintln!("xtask test: {e}\n\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    match execute(&opts) {
        Ok(results) if results.iter().any(|r| r.status.is_failure()) => ExitCode::from(1),
        Ok(_) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("xtask test: error: {e}");
            ExitCode::from(2)
        }
    }
}

/// The repository root (this crate lives in `<root>/xtask`).
pub fn repo_root() -> PathBuf {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest.parent().unwrap_or(manifest).to_path_buf()
}

/// Runs the selected tests, printing results and the summary to stdout.
/// `Err` is an infrastructure problem (exit status 2).
pub fn execute(opts: &Options) -> Result<Vec<TestResult>, String> {
    let start = Instant::now();
    let root = repo_root();
    let target = target_dir(&root)?;

    let inputs: Vec<PathBuf> = if opts.paths.is_empty() {
        discover::default_roots(&root)
    } else {
        opts.paths
            .iter()
            .map(|p| if p.exists() || p.is_absolute() { p.clone() } else { root.join(p) })
            .collect()
    };
    let cases = discover::discover(&root, &inputs, opts.filter.as_deref())?;
    if cases.is_empty() {
        return Err("no tests selected".into());
    }

    let compiler = match &opts.compiler {
        Some(c) => c.clone(),
        None => build_kiln(&root, &target)?,
    };
    let compiler_kind = detect_kind(&compiler)?;
    let oracle = if opts.no_oracle {
        None
    } else {
        detect_kind(&opts.oracle)
            .map_err(|e| format!("{e} (pass --no-oracle to skip oracle comparisons)"))?;
        Some(opts.oracle.clone())
    };

    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_nanos());
    let scratch = target.join("xtask-test").join(format!("run-{}-{nanos}", std::process::id()));
    std::fs::create_dir_all(&scratch)
        .map_err(|e| format!("cannot create {}: {e}", scratch.display()))?;

    let cfg = runner::Config {
        root,
        compiler,
        compiler_kind,
        oracle,
        update: opts.update,
        scratch,
        compile_timeout: Duration::from_secs(120),
        run_timeout: opts.run_timeout,
    };
    let jobs = opts
        .jobs
        .unwrap_or_else(|| std::thread::available_parallelism().map_or(1, |n| n.get().min(8)))
        .min(cases.len());
    println!(
        "running {} tests with {} ({}), oracle: {}, {jobs} jobs",
        cases.len(),
        cfg.compiler.display(),
        compiler_kind.as_str(),
        cfg.oracle.as_ref().map_or_else(|| "none".to_owned(), |o| o.display().to_string()),
    );

    let results = run_all(&cfg, &cases, jobs);
    let _ = std::fs::remove_dir_all(&cfg.scratch);

    // `print!` (not a raw stdout handle) so libtest captures it in unit tests.
    let mut text = String::new();
    for r in &results {
        text.push_str(&report::format_result(r, opts.verbose));
    }
    let updated = results.iter().filter(|r| r.updated).count();
    if updated > 0 {
        let _ = writeln!(text, "updated {updated} .expected file(s)");
    }
    let counts = report::Counts::of(&results);
    text.push_str(&report::format_summary(&counts, results.len(), start.elapsed()));
    print!("{text}");
    Ok(results)
}

fn run_all(cfg: &runner::Config, cases: &[discover::TestCase], jobs: usize) -> Vec<TestResult> {
    let next = AtomicUsize::new(0);
    let slots: Mutex<Vec<Option<TestResult>>> = Mutex::new(vec![None; cases.len()]);
    std::thread::scope(|s| {
        for _ in 0..jobs.max(1) {
            s.spawn(|| {
                loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    let Some(case) = cases.get(i) else { break };
                    let r = runner::run_test(cfg, case, i);
                    slots.lock().expect("no worker panics while holding the lock")[i] = Some(r);
                }
            });
        }
    });
    slots
        .into_inner()
        .expect("workers have finished")
        .into_iter()
        .map(|r| r.expect("every test produced a result"))
        .collect()
}

/// `--version` tells kiln (`kiln version ...`) from clang (`... clang version ...`).
fn detect_kind(compiler: &Path) -> Result<CompilerKind, String> {
    let o = process::run(Command::new(compiler).arg("--version"), Duration::from_secs(30))
        .map_err(|e| format!("cannot run compiler `{}`: {e}", compiler.display()))?;
    if !o.success() {
        return Err(format!("`{} --version` failed ({})", compiler.display(), o.describe_end()));
    }
    let first = o.stdout_str().lines().next().unwrap_or_default().to_owned();
    Ok(if first.starts_with("kiln version") {
        CompilerKind::Kiln
    } else if first.contains("clang version") {
        CompilerKind::Clang
    } else {
        CompilerKind::Other
    })
}

/// `$CARGO_TARGET_DIR`, else `target_directory` from `cargo metadata`.
fn target_dir(root: &Path) -> Result<PathBuf, String> {
    if let Some(d) = std::env::var_os("CARGO_TARGET_DIR").filter(|d| !d.is_empty()) {
        return Ok(root.join(d));
    }
    let o = Command::new(env!("CARGO"))
        .args(["metadata", "--format-version", "1", "--no-deps"])
        .current_dir(root)
        .output()
        .map_err(|e| format!("cannot run cargo metadata: {e}"))?;
    if !o.status.success() {
        return Err(format!("cargo metadata failed: {}", String::from_utf8_lossy(&o.stderr)));
    }
    json_string_field(&String::from_utf8_lossy(&o.stdout), "target_directory")
        .map(PathBuf::from)
        .ok_or_else(|| "cargo metadata: no target_directory".into())
}

/// Extracts a top-level-ish string field from JSON without a JSON library.
/// Sufficient for `cargo metadata`, whose `target_directory` key is unique.
fn json_string_field(json: &str, key: &str) -> Option<String> {
    let needle = format!("\"{key}\":");
    let rest = json[json.find(&needle)? + needle.len()..].trim_start();
    let mut chars = rest.strip_prefix('"')?.chars();
    let mut out = String::new();
    loop {
        match chars.next()? {
            '"' => return Some(out),
            '\\' => match chars.next()? {
                'n' => out.push('\n'),
                't' => out.push('\t'),
                'u' => {
                    let hex: String = chars.by_ref().take(4).collect();
                    out.push(char::from_u32(u32::from_str_radix(&hex, 16).ok()?)?);
                }
                c => out.push(c), // \" \\ \/
            },
            c => out.push(c),
        }
    }
}

/// `cargo build -p kiln_driver`, returning the path of the `kiln` binary.
fn build_kiln(root: &Path, target: &Path) -> Result<PathBuf, String> {
    eprintln!("xtask test: building kiln (cargo build -p kiln_driver)");
    let status = Command::new(env!("CARGO"))
        .args(["build", "--quiet", "-p", "kiln_driver"])
        .current_dir(root)
        .status()
        .map_err(|e| format!("cannot run cargo build: {e}"))?;
    if !status.success() {
        return Err(format!("cargo build -p kiln_driver failed ({status})"));
    }
    let exe = target.join("debug").join(format!("kiln{}", std::env::consts::EXE_SUFFIX));
    if exe.is_file() { Ok(exe) } else { Err(format!("kiln binary not found at {}", exe.display())) }
}

#[cfg(test)]
mod tests;
