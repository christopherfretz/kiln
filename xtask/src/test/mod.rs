//! `cargo xtask test`: the conformance/differential test runner.
//!
//! The contract (CLI, directive semantics, exit codes, output) is
//! `spec/harness.md`; the directive format is `docs/VALIDATION.md`.

mod cfg_shape;
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
  --meta-header       header-differential mode: run the tests that include
                      tests/support/kiln_meta*.h again with libstdc++'s real
                      <meta> (the GCC oracle's installation, --gcc-toolchain)
                      in its place; no clang oracle, no oven:/inst-report:
                      budgets (spec/harness.md \"Header-differential mode\")
  --debug-sweep       debug-info sweep: compile each compile-ok, run, ir, layout,
                      vtables and debugger test to an object without and then
                      with -g; one that compiles without -g must compile with
                      it, to DWARF that llvm-dwarfdump --verify accepts; no
                      oracles, xfail ignored (spec/harness.md \"Debug-info sweep\")
  --external <dir>    run every .c/.cpp under <dir> (outside the repo) as
                      never-crash: counts only, nothing written (spec/harness.md)
  --filter <substr>   only tests whose path contains <substr>
  --shard <K/N>       run only shard K of N (1 <= K <= N): test i (0-based) of the
                      sorted list (after --filter) runs iff i % N == K-1, so shards
                      1..N together are exactly the unsharded run (CI fan-out)
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
  KILN_REQUIRE_GCC    1: no usable GCC oracle is an error (exit 2), not dropped
                      comparisons (GitHub CI sets it; --no-gcc/--no-oracle then too)
  KILN_REQUIRE_DEBUGGERS
                      1: an `expect: debugger` test whose gdb or lldb-19 is not on
                      PATH is an ERROR, not a SKIP (GitHub CI sets it)

exit status: 0 all passed (xfail/skip allowed), 1 a test failed/xpassed/errored/was killed,
2 usage or infrastructure error.
";

/// `--shard K/N` (spec/harness.md "Command line"): of the sorted,
/// filtered test list, test `i` (0-based) is in shard `K` iff
/// `i % N == K - 1`. Round-robin rather than contiguous blocks, so a slow
/// directory is spread over every shard; the shards of one `N` partition
/// the list (no test is lost or run twice).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Shard {
    /// `K`, 1-based.
    pub index: usize,
    /// `N`, at least 1.
    pub count: usize,
}

impl Shard {
    /// Parses `K/N` with `1 <= K <= N`.
    pub fn parse(s: &str) -> Result<Self, String> {
        let bad = || format!("bad --shard `{s}` (K/N with 1 <= K <= N)");
        let (k, n) = s.split_once('/').ok_or_else(bad)?;
        let index: usize = k.trim().parse().map_err(|_| bad())?;
        let count: usize = n.trim().parse().map_err(|_| bad())?;
        if index == 0 || index > count {
            return Err(bad());
        }
        Ok(Self { index, count })
    }

    /// Does test `i` (0-based, in sorted order) belong to this shard?
    pub fn keeps(self, i: usize) -> bool {
        i % self.count == self.index - 1
    }

    /// The tests of `cases` (sorted) in this shard, in order.
    pub fn select<T>(self, cases: Vec<T>) -> Vec<T> {
        cases.into_iter().enumerate().filter(|&(i, _)| self.keeps(i)).map(|(_, c)| c).collect()
    }
}

impl std::fmt::Display for Shard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{}", self.index, self.count)
    }
}

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
    /// `KILN_REQUIRE_GCC=1`: no usable GCC oracle is an infrastructure
    /// error. Not a flag, and not read by [`parse_args`] or [`execute`]: the
    /// entry points ([`main`], `xtask ci`) set it from the environment, so
    /// the harness's own tests are unaffected by CI's.
    pub require_gcc: bool,
    /// `KILN_REQUIRE_DEBUGGERS=1`: a `debugger` test whose debugger is
    /// missing is an ERROR, not a SKIP. Set by the entry points from the
    /// environment, as [`Options::require_gcc`].
    pub require_debuggers: bool,
    pub no_oracle: bool,
    /// `--meta-header`: the header-differential mode.
    pub meta_header: bool,
    /// `--debug-sweep`: the debug-info sweep.
    pub debug_sweep: bool,
    /// `--bmi-kind 1|2`: the module-file variant (`spec/harness.md`
    /// "Module-file kinds"): only `module:` tests, kiln always two-step,
    /// writing module files of this kind.
    pub bmi_kind: Option<u8>,
    /// `--bmi-image`: with `--bmi-kind 2` (which it implies), kiln writes
    /// and reads the declaration image (`-fkiln-bmi-image`,
    /// spec/modules.md §11.11 "Part 2a").
    pub bmi_image: bool,
    /// `--external <dir>`: the never-crash run of [`external`].
    pub external: Option<PathBuf>,
    pub filter: Option<String>,
    /// `--shard K/N`: only every N-th test of the sorted list, from K-1.
    pub shard: Option<Shard>,
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
            require_gcc: false,
            require_debuggers: false,
            no_oracle: false,
            meta_header: false,
            debug_sweep: false,
            bmi_kind: None,
            bmi_image: false,
            external: None,
            filter: None,
            shard: None,
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
            "--shard" => o.shard = Some(Shard::parse(&value("--shard")?)?),
            "--bmi-image" => o.bmi_image = true,
            "--bmi-kind" => {
                let v = value("--bmi-kind")?;
                o.bmi_kind = Some(match v.as_str() {
                    "1" => 1,
                    "2" => 2,
                    _ => return Err(format!("bad --bmi-kind `{v}` (1|2)")),
                });
            }
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
            "--no-oracle" | "--no-gcc" | "--meta-header" | "--debug-sweep" | "--update" | "-v"
            | "--verbose"
                if inline.is_some() =>
            {
                return Err(format!("`{flag}` takes no value"));
            }
            "--no-oracle" => o.no_oracle = true,
            "--no-gcc" => o.no_gcc = true,
            "--meta-header" => o.meta_header = true,
            "--debug-sweep" => o.debug_sweep = true,
            "--update" => o.update = true,
            "-v" | "--verbose" => o.verbose = true,
            f if f.starts_with('-') => return Err(format!("unknown option `{f}`")),
            _ => o.paths.push(PathBuf::from(arg)),
        }
    }
    if o.bmi_image {
        match o.bmi_kind {
            None | Some(2) => o.bmi_kind = Some(2),
            Some(_) => return Err("--bmi-image needs --bmi-kind 2".into()),
        }
    }
    Ok(Some(o))
}

/// The debug-info sweep's DWARF verifier: `llvm-dwarfdump` of the oracle
/// release's LLVM, else `llvm-dwarfdump-N` on `PATH` (`None`: the sweep
/// only compiles).
fn debug_sweep_dwarfdump(llvm_dir: &Path) -> Option<PathBuf> {
    let own = llvm_dir.join("bin").join("llvm-dwarfdump");
    if own.is_file() {
        return Some(own);
    }
    let name = llvm_dir.file_name()?.to_str()?.replace("llvm-", "llvm-dwarfdump-");
    std::env::var_os("PATH")
        .and_then(|p| std::env::split_paths(&p).map(|d| d.join(&name)).find(|f| f.is_file()))
}

/// Entry point for `cargo xtask test`.
pub fn main(args: impl IntoIterator<Item = String>) -> ExitCode {
    let mut opts = match parse_args(args) {
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
        if opts.shard.is_some() {
            eprintln!("xtask test: `--shard` does not apply to `--external`\n\n{USAGE}");
            return ExitCode::from(2);
        }
        return external::main(&opts);
    }
    opts.require_gcc = crate::oracle::require_gcc_from_env();
    opts.require_debuggers = require_debuggers_from_env();
    match execute(&opts) {
        Ok(results) if results.iter().any(|r| r.status.is_failure()) => ExitCode::from(1),
        Ok(_) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("xtask test: error: {e}");
            ExitCode::from(2)
        }
    }
}

/// The stand-in header directory of the header-differential mode
/// (`--meta-header`), which needs the GCC oracle's installation prefix.
fn meta_header_dir(
    opts: &Options,
    root: &Path,
    gcc_toolchain: Option<&Path>,
) -> Result<Option<PathBuf>, String> {
    if !opts.meta_header {
        return Ok(None);
    }
    if gcc_toolchain.is_none() {
        return Err("--meta-header needs a GCC oracle (--gcc / KILN_GCC) whose \
                    installation prefix holds libstdc++'s <meta>"
            .into());
    }
    Ok(Some(root.join("tests").join("support").join("meta-header")))
}

/// Runs the selected tests, printing results and the summary to stdout.
/// `Err` is an infrastructure problem (exit status 2).
#[allow(clippy::too_many_lines, reason = "one linear setup / run / report sequence")]
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
    let mut cases = discover::discover(&root, &inputs, opts.filter.as_deref())?;
    if opts.bmi_kind.is_some() {
        // The module-file variant runs only tests with `module:` lines
        // (the runner skips any other, should one slip through).
        cases.retain(|c| std::fs::read_to_string(&c.path).is_ok_and(|t| t.contains("//@ module:")));
    }
    if cases.is_empty() {
        return Err("no tests selected".into());
    }
    // After the emptiness check: a shard of a list shorter than N may be
    // empty, and that is not an error (its tests are in the other shards).
    let selected = cases.len();
    let cases = match opts.shard {
        Some(shard) => shard.select(cases),
        None => cases,
    };

    let compiler = match &opts.compiler {
        Some(c) => c.clone(),
        None => build_kiln(&root, &target, "test", false)?,
    };
    let compiler_id = crate::util::identify(&compiler)?;
    let compiler_kind = compiler_id.kind;
    if opts.meta_header && compiler_kind != CompilerKind::Kiln {
        return Err("--meta-header runs kiln only (the compiler is not kiln)".into());
    }
    if opts.bmi_kind.is_some() && compiler_kind != CompilerKind::Kiln {
        return Err("--bmi-kind runs kiln only (the compiler is not kiln)".into());
    }
    if opts.meta_header && opts.debug_sweep {
        return Err("--meta-header and --debug-sweep do not combine".into());
    }
    let (oracle, oracle_kind, oracle_clang) =
        if opts.no_oracle || opts.meta_header || opts.debug_sweep {
            (None, CompilerKind::Other, None)
        } else {
            let oracle = opts.oracle.clone().unwrap_or_else(|| PathBuf::from(toolchain.clang()));
            let id = crate::util::identify(&oracle)
                .map_err(|e| format!("{e} (pass --no-oracle to skip oracle comparisons)"))?;
            (Some(oracle), id.kind, id.clang_version())
        };
    // The sweep gives no GCC verdicts, but `%{gcc_toolchain}` still needs GCC.
    let gcc = gcc_oracle(opts)?;
    let gcc_toolchain = gcc.as_ref().and_then(|(path, _)| crate::oracle::gcc_toolchain_of(path));
    let meta_header = meta_header_dir(opts, &root, gcc_toolchain.as_deref())?;

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
        gcc: gcc.as_ref().map(|(path, _)| path.clone()),
        gcc_toolchain,
        meta_header,
        llvm_dir: toolchain.llvm_dir(),
        llvm_major: toolchain.version,
        update: opts.update,
        scratch,
        compile_timeout: Duration::from_secs(120),
        run_timeout: opts.run_timeout,
        pp_compare: opts.pp_compare,
        mem_limit: process::MemLimit::from_mib(opts.mem_limit_mib),
        require_debuggers: opts.require_debuggers,
        debug_sweep: opts.debug_sweep.then(|| runner::DebugSweep {
            dwarfdump: debug_sweep_dwarfdump(&toolchain.llvm_dir()),
        }),
        bmi_kind: opts.bmi_kind,
        bmi_image: opts.bmi_image,
    };
    let jobs = opts
        .jobs
        .unwrap_or_else(|| std::thread::available_parallelism().map_or(1, |n| n.get().min(8)))
        .min(cases.len().max(1));
    println!(
        "running {} tests{} with {} ({}), oracle: {}, gcc: {}, {jobs} jobs{}, mem limit: {}",
        cases.len(),
        opts.shard.map_or_else(String::new, |s| format!(" (shard {s} of {selected})")),
        cfg.compiler.display(),
        compiler_kind.as_str(),
        cfg.oracle.as_ref().map_or_else(|| "none".to_owned(), |o| o.display().to_string()),
        gcc.as_ref().map_or_else(
            || "none".to_owned(),
            |(path, version)| format!("{} ({version})", path.display())
        ),
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

/// The environment variable that makes the debuggers of `expect: debugger`
/// tests mandatory (`spec/harness.md` "Debugger tests").
pub const REQUIRE_DEBUGGERS_ENV: &str = "KILN_REQUIRE_DEBUGGERS";

/// Whether a `KILN_REQUIRE_DEBUGGERS` value requires the debuggers: exactly
/// `1`, as `KILN_REQUIRE_GCC`.
pub fn require_debuggers_from(value: Option<&str>) -> bool {
    value.map(str::trim) == Some("1")
}

/// [`require_debuggers_from`] of the environment. Only the entry points
/// read it, so the harness's own tests never see CI's.
pub fn require_debuggers_from_env() -> bool {
    require_debuggers_from(std::env::var(REQUIRE_DEBUGGERS_ENV).ok().as_deref())
}

/// The GCC oracle of this run and its `--version` line: [`resolve_gcc`],
/// unless `--no-gcc`/`--no-oracle` drop it. With [`Options::require_gcc`]
/// (`KILN_REQUIRE_GCC=1`) a run without one is an infrastructure error, so
/// CI never passes with its `oracle: gcc` comparisons silently skipped.
pub fn gcc_oracle(opts: &Options) -> Result<Option<(PathBuf, String)>, String> {
    gcc_oracle_with(opts, std::env::var(crate::oracle::GCC_ENV).ok().as_deref())
}

/// [`gcc_oracle`] with `kiln_gcc` as the value of `KILN_GCC`.
fn gcc_oracle_with(
    opts: &Options,
    kiln_gcc: Option<&str>,
) -> Result<Option<(PathBuf, String)>, String> {
    let gcc = if opts.no_oracle || opts.no_gcc {
        None
    } else {
        resolve_gcc(opts.gcc.as_deref(), kiln_gcc)?
    };
    if gcc.is_none() && opts.require_gcc {
        let why = if opts.no_oracle || opts.no_gcc {
            "`--no-gcc`/`--no-oracle` drop it".to_owned()
        } else {
            format!(
                "set {} or --gcc to GCC 16.1, or put {} on PATH",
                crate::oracle::GCC_ENV,
                crate::oracle::GCC_DEFAULT
            )
        };
        return Err(format!(
            "{}=1 and no usable GCC oracle for `oracle: gcc` tests ({why}; spec/harness.md \"GCC \
             oracle\")",
            crate::oracle::REQUIRE_GCC_ENV
        ));
    }
    Ok(gcc)
}

/// The GCC oracle: `--gcc`, else `KILN_GCC`, else `g++-16` (`spec/harness.md`
/// "GCC oracle"), with its `--version` line. An explicitly named GCC that
/// does not run, or is not GCC, is an infrastructure error; the default one
/// missing is `None` (the GCC comparisons are dropped, never failed).
fn resolve_gcc(
    flag: Option<&Path>,
    kiln_gcc: Option<&str>,
) -> Result<Option<(PathBuf, String)>, String> {
    let (path, explicit) = match flag {
        Some(p) => (p.to_path_buf(), true),
        None => crate::oracle::gcc_from(kiln_gcc),
    };
    match crate::util::identify(&path) {
        Ok(id) if id.kind == CompilerKind::Gcc => Ok(Some((path, id.version))),
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
