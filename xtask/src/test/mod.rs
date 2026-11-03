//! `cargo xtask test`: the conformance/differential test runner.
//!
//! The contract (CLI, directive semantics, exit codes, output) is
//! `spec/harness.md`; the directive format is `docs/VALIDATION.md`.

mod compare;
mod diag_parse;
pub(crate) mod directives;
mod discover;
pub(crate) mod external;
mod inst_report;
mod pp_tokens;
mod report;
mod runner;

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use directives::PpCompare;
pub use report::{CorpusOutcome, ci_corpus_passes, ci_should_retry};
use runner::TestResult;

#[cfg(test)]
use crate::util::json_string_field;
use crate::util::{CompilerKind, build_kiln, process};
pub use crate::util::{repo_root, target_dir};

pub const USAGE: &str = "\
usage: cargo xtask test [options] [paths...]

Runs tests/conformance, tests/differential and tests/stress (or the given
files/directories) through a compiler. See spec/harness.md.

options:
  --compiler <path>   compiler under test (default: build and use the kiln++ binary)
  --oracle <path>     oracle compiler for run/pp comparisons (default: clang++-N,
                      N from KILN_ORACLE)
  --gcc <path>        the GCC oracle of `oracle: gcc` tests (default: $KILN_GCC,
                      else g++-16 on PATH if it runs; none: their GCC comparison
                      is dropped)
  --no-gcc            drop the GCC comparisons only
  --no-oracle         skip all oracle comparisons (clang and GCC)
  --external <dir>    run every .c/.cpp under <dir> (outside the repo) as
                      never-crash: counts only, nothing written (spec/harness.md)
  --filter <substr>   only tests whose path contains <substr>
  --update            rewrite existing pp `.expected` files with the actual output
                      (only those that differ under the comparison rule)
  --pp-compare <how>  compare pp output as `tokens` or `text`, overriding every
                      test's `pp-compare:` directive
  --report-parity <file>
                      write per-directory pass/fail counts into <file> between
                      its `<!-- parity:begin -->`/`<!-- parity:end -->` markers
  -j, --jobs <N>      parallel tests (default: min(available CPUs, 8)); an upper
                      bound when MAKEFLAGS names a jobserver fifo, whose tokens
                      every spawned process then also holds (spec/harness.md)
  --timeout <secs>    run-time limit for test binaries (default: 10)
  --mem-limit <MiB>   address-space cap of every compiler, oracle and test
                      binary process (default: 8192; 0 disables it, leaving
                      only `mem<=N` budgets); kiln's stack reservation is added
  -v, --verbose       print every result and the commands that were run
  -h, --help          this text

environment:
  KILN_ORACLE         the oracle clang/LLVM release, 22 (default) or 19: the default
                      --oracle is clang++-N, and kiln++ runs with
                      KILN_LLVM_DIR=/usr/lib/llvm-N (spec/harness.md \"Oracle toolchain\")
  KILN_GCC            the GCC oracle (default g++-16; spec/harness.md \"GCC oracle\")

exit status: 0 all passed (xfail/skip allowed), 1 a test failed/xpassed/errored/was killed,
2 usage or infrastructure error.
";

/// Parsed command line.
#[derive(Clone, Debug, PartialEq)]
#[allow(clippy::struct_excessive_bools)] // independent command-line switches
pub struct Options {
    pub compiler: Option<PathBuf>,
    /// `--oracle`; `None` is `clang++-N` of [`crate::oracle::Oracle::from_env`].
    pub oracle: Option<PathBuf>,
    /// `--gcc`; `None` is `KILN_GCC`, else `g++-16` when it runs.
    pub gcc: Option<PathBuf>,
    /// `--no-gcc`: drop the GCC comparisons (the clang oracle stays).
    pub no_gcc: bool,
    pub no_oracle: bool,
    /// `--external <dir>`: the never-crash run of [`external`].
    pub external: Option<PathBuf>,
    pub filter: Option<String>,
    pub update: bool,
    pub jobs: Option<usize>,
    pub run_timeout: Duration,
    /// `--mem-limit`, in MiB; `0` disables the default cap.
    pub mem_limit_mib: u64,
    pub verbose: bool,
    pub pp_compare: Option<PpCompare>,
    pub report_parity: Option<PathBuf>,
    pub paths: Vec<PathBuf>,
    /// The LLVM release for `requires: llvm` and kiln's `KILN_LLVM_DIR`
    /// (and the default `--oracle`); `None` is
    /// [`crate::oracle::Oracle::from_env`]. Not a flag: the harness's own
    /// tests pin it, so their outcome does not depend on `KILN_ORACLE`
    /// ("CI", `--oracle-only`).
    pub toolchain: Option<crate::oracle::Oracle>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            compiler: None,
            oracle: None,
            gcc: None,
            no_gcc: false,
            no_oracle: false,
            external: None,
            filter: None,
            update: false,
            jobs: None,
            run_timeout: Duration::from_secs(10),
            mem_limit_mib: process::DEFAULT_MEM_LIMIT_MIB,
            verbose: false,
            pp_compare: None,
            report_parity: None,
            paths: Vec::new(),
            toolchain: None,
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
            "--oracle" => o.oracle = Some(PathBuf::from(value("--oracle")?)),
            "--gcc" => o.gcc = Some(PathBuf::from(value("--gcc")?)),
            "--external" => o.external = Some(PathBuf::from(value("--external")?)),
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
            "--mem-limit" => {
                let v = value("--mem-limit")?;
                o.mem_limit_mib =
                    v.parse().map_err(|_| format!("bad --mem-limit `{v}` (MiB; 0 disables)"))?;
            }
            "--pp-compare" => {
                let v = value("--pp-compare")?;
                o.pp_compare = Some(
                    PpCompare::parse(&v)
                        .ok_or_else(|| format!("bad --pp-compare `{v}` (tokens|text)"))?,
                );
            }
            "--report-parity" => o.report_parity = Some(PathBuf::from(value("--report-parity")?)),
            "--no-oracle" | "--no-gcc" | "--update" | "-v" | "--verbose" if inline.is_some() => {
                return Err(format!("`{flag}` takes no value"));
            }
            "--no-oracle" => o.no_oracle = true,
            "--no-gcc" => o.no_gcc = true,
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
    if opts.external.is_some() {
        return external::main(&opts);
    }
    match execute(&opts) {
        Ok(results) if results.iter().any(|r| r.status.is_failure()) => ExitCode::from(1),
        Ok(_) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("xtask test: error: {e}");
            ExitCode::from(2)
        }
    }
}

/// Runs the selected tests, printing results and the summary to stdout.
/// `Err` is an infrastructure problem (exit status 2).
pub fn execute(opts: &Options) -> Result<Vec<TestResult>, String> {
    let start = Instant::now();
    // First, so a bad `KILN_ORACLE` fails before anything is built or run.
    let toolchain = match opts.toolchain {
        Some(t) => t,
        None => crate::oracle::Oracle::from_env()?,
    };
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
        None => build_kiln(&root, &target, "test", false)?,
    };
    let compiler_id = crate::util::identify(&compiler)?;
    let compiler_kind = compiler_id.kind;
    let (oracle, oracle_kind, oracle_clang) = if opts.no_oracle {
        (None, CompilerKind::Other, None)
    } else {
        let oracle = opts.oracle.clone().unwrap_or_else(|| PathBuf::from(toolchain.clang()));
        let id = crate::util::identify(&oracle)
            .map_err(|e| format!("{e} (pass --no-oracle to skip oracle comparisons)"))?;
        (Some(oracle), id.kind, id.clang_version())
    };
    let gcc = if opts.no_oracle || opts.no_gcc { None } else { resolve_gcc(opts.gcc.as_deref())? };

    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_nanos());
    let scratch = target.join("xtask-test").join(format!("run-{}-{nanos}", std::process::id()));
    std::fs::create_dir_all(&scratch)
        .map_err(|e| format!("cannot create {}: {e}", scratch.display()))?;

    let cfg = runner::Config {
        root,
        compiler,
        compiler_kind,
        compiler_clang: compiler_id.clang_version(),
        oracle,
        oracle_kind,
        oracle_clang,
        gcc,
        llvm_dir: toolchain.llvm_dir(),
        llvm_major: toolchain.version,
        update: opts.update,
        scratch,
        compile_timeout: Duration::from_secs(120),
        run_timeout: opts.run_timeout,
        pp_compare: opts.pp_compare,
        mem_limit: process::MemLimit::from_mib(opts.mem_limit_mib),
    };
    let jobs = opts
        .jobs
        .unwrap_or_else(|| std::thread::available_parallelism().map_or(1, |n| n.get().min(8)))
        .min(cases.len());
    println!(
        "running {} tests with {} ({}), oracle: {}, gcc: {}, {jobs} jobs{}, mem limit: {}",
        cases.len(),
        cfg.compiler.display(),
        compiler_kind.as_str(),
        cfg.oracle.as_ref().map_or_else(|| "none".to_owned(), |o| o.display().to_string()),
        cfg.gcc.as_ref().map_or_else(|| "none".to_owned(), |o| o.display().to_string()),
        crate::jobs::global().describe().map_or_else(String::new, |d| format!(" (jobserver {d})")),
        cfg.mem_limit.map_or_else(|| "none".to_owned(), |l| format!("{} MiB", l.mib)),
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
    if let Some(path) = &opts.report_parity {
        let path = if path.is_absolute() { path.clone() } else { cfg.root.join(path) };
        let header = report::ParityHeader {
            compiler: compiler_id.version.clone(),
            oracle: cfg.oracle.as_ref().map(|o| o.display().to_string()),
            pp_compare: opts.pp_compare,
            date: SystemTime::now(),
            elapsed: start.elapsed(),
        };
        report::write_parity(&path, &header, &results)?;
        println!("wrote parity report to {}", path.display());
    }
    Ok(results)
}

/// The GCC oracle: `--gcc`, else `KILN_GCC`, else `g++-16` (`spec/harness.md`
/// "GCC oracle"). An explicitly named GCC that does not run, or is not GCC,
/// is an infrastructure error; the default one missing is `None` (the GCC
/// comparisons are dropped, never failed).
fn resolve_gcc(flag: Option<&Path>) -> Result<Option<PathBuf>, String> {
    let (path, explicit) = match flag {
        Some(p) => (p.to_path_buf(), true),
        None => crate::oracle::gcc_from_env(),
    };
    match crate::util::identify(&path) {
        Ok(id) if id.kind == CompilerKind::Gcc => Ok(Some(path)),
        Ok(id) if explicit => Err(format!(
            "the GCC oracle `{}` is not GCC (`--version`: {})",
            path.display(),
            id.version
        )),
        Err(e) if explicit => {
            Err(format!("{e} (the GCC oracle; unset {} to drop it)", crate::oracle::GCC_ENV))
        }
        _ => Ok(None),
    }
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
#[cfg(test)]
fn detect_kind(compiler: &Path) -> Result<CompilerKind, String> {
    crate::util::identify(compiler).map(|id| id.kind)
}

#[cfg(test)]
mod tests;
