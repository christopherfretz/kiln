//! `cargo xtask test`: the conformance/differential test runner.
//!
//! The contract (CLI, directive semantics, exit codes, output) is
//! `spec/harness.md`; the directive format is `docs/VALIDATION.md`.

mod compare;
mod diag_parse;
pub(crate) mod directives;
mod discover;
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
pub use report::{CorpusOutcome, ci_corpus_passes};
use runner::TestResult;

#[cfg(test)]
use crate::util::json_string_field;
use crate::util::{CompilerKind, build_kiln};
pub use crate::util::{repo_root, target_dir};

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
                      (only those that differ under the comparison rule)
  --pp-compare <how>  compare pp output as `tokens` or `text`, overriding every
                      test's `pp-compare:` directive
  --report-parity <file>
                      write per-directory pass/fail counts into <file> between
                      its `<!-- parity:begin -->`/`<!-- parity:end -->` markers
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
    pub pp_compare: Option<PpCompare>,
    pub report_parity: Option<PathBuf>,
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
            pp_compare: None,
            report_parity: None,
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
            "--pp-compare" => {
                let v = value("--pp-compare")?;
                o.pp_compare = Some(
                    PpCompare::parse(&v)
                        .ok_or_else(|| format!("bad --pp-compare `{v}` (tokens|text)"))?,
                );
            }
            "--report-parity" => o.report_parity = Some(PathBuf::from(value("--report-parity")?)),
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
        None => build_kiln(&root, &target, "test", false)?,
    };
    let compiler_id = crate::util::identify(&compiler)?;
    let compiler_kind = compiler_id.kind;
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
        pp_compare: opts.pp_compare,
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
    crate::util::identify(compiler).map(|id| id.kind)
}

#[cfg(test)]
mod tests;
