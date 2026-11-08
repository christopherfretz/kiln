//! `cargo xtask bench`: the performance harness.
//!
//! The contract (CLI, directives, records, regression policy) is
//! `spec/bench.md`; the policy background is `docs/VALIDATION.md`.

mod compare;
pub(crate) mod entry;
mod perf_md;
pub(crate) mod record;
mod scaling;
mod slice;
mod stats;
#[cfg(test)]
mod tests;

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use compare::{RatioRow, ThresholdOverrides, Thresholds, Verdict};
use entry::{BenchSpec, Entry};
use record::{
    Baseline, BaselineFile, CompilerRun, Host, InstCounters, Measurement, Outcome, RunRecord,
};
use stats::{Summary, round_to};

use crate::util::oven_report::{self, Counters};
use crate::util::process::{self, RssProbe, Shown};
use crate::util::{CompilerId, CompilerKind, build_kiln, identify, repo_root, target_dir};

pub const USAGE: &str = "\
usage: cargo xtask bench [options] [paths...]

Measures compile time and peak memory of the bench/ entries (or the given
files/directories under bench/) and compares them with bench/baselines.
See spec/bench.md.

entries: every bench/<group>/<name>.cpp, plus, only when named explicitly,
  bench/libcxx-slice: the libc++ tests of bench/libcxx-slice/slice.list at
                      -fsyntax-only with the flags `cargo xtask libcxx` uses
                      (record-only: per-test and aggregate ratios against
                      --compare-with, e.g. clang++-22; never judged against
                      baselines)
  bench/reflect:      the reflection shapes S1-S9 of `cargo xtask gen-reflect`

off-queue modes (spec/bench.md, Scaling sweeps and External corpora):
  --scaling <shape>        compile gen-reflect shape s1..s9 at N and 4N; report
                           wall, peak RSS, the constexpr counters and the 4N/N
                           ratios; fail above 4.6x or when peak_arena grows on
                           s1/s2/s4/s5/s8 (writes no records)
  --n <N>                  N of --scaling (default: the shape's default N)
  --external <dir>         compile every .c/.cpp under <dir> (outside the repo)
                           once, serially, never-crash: print verdict counts,
                           total compile time and the largest peak RSS only

options:
  --compiler <path>        compiler to measure (default: build and use a release kiln++)
  --baseline <dir>         baseline directory (default: bench/baselines)
  --record                 write the measurements as the new baselines; with
                           --compare-with, also merge the ratio table into
                           docs/status/perf.md
  --compare-with <path>    also measure this compiler (e.g. clang++-22, the libc++
                           slice's oracle, or clang++-19) and print
                           the time and memory ratios
  --filter <substr>        only entries whose id (<group>/<name>) contains <substr>
  --runs <N>               runs per entry (default: 5; the fastest and slowest are
                           dropped when N >= 3)
  --threshold-time <PCT>   allowed wall-clock regression in percent (default: 5)
  --threshold-mem <PCT>    allowed peak-RSS regression in percent (default: 2)
  --threshold-steps <PCT>  allowed constexpr-steps regression in percent (kiln
                           only; default: 0)
  --threshold-bytes <PCT>  allowed constexpr bytes/peak_bytes/peak_arena/
                           frozen_bytes regression in percent (kiln only;
                           default: 2)
                           (each --threshold-* overrides the entries'
                           threshold-* directives, which override the defaults)
  --mem-limit <MiB>        address-space cap of every measured compile (default:
                           8192; 0 disables it); kiln's stack reservation is added
  -v, --verbose            print the commands that were run
  -h, --help               this text

exit status: 0 no failures or regressions, 1 an entry failed or regressed beyond a
threshold (regressions are not failures with --record), 2 usage or
infrastructure error.
";

/// Per-run limit. Bench entries take seconds; this only catches hangs.
const RUN_TIMEOUT: Duration = Duration::from_secs(300);
/// A median this far above the minimum is reported as noisy.
const NOISY_SPREAD_PCT: f64 = 10.0;

/// The group whose kiln measurements record the `inst.*` metrics.
const INST_GROUP: &str = "frontend";
/// kiln's instantiation report, as NDJSON on stderr (its last line is the
/// totals row, [`InstCounters::parse_report`]).
const INST_FLAG: &str = "-fkiln-instantiation-report=json";

/// Parsed command line.
#[derive(Clone, Debug, PartialEq)]
pub struct Options {
    pub compiler: Option<PathBuf>,
    pub baseline: Option<PathBuf>,
    pub record: bool,
    pub compare_with: Option<PathBuf>,
    pub filter: Option<String>,
    pub runs: usize,
    /// `--threshold-*`; they override the entries' `threshold-*:` directives.
    pub thresholds: ThresholdOverrides,
    /// `--mem-limit`, in MiB; `0` disables the cap.
    pub mem_limit_mib: u64,
    pub verbose: bool,
    pub paths: Vec<PathBuf>,
    /// `--scaling <shape>` and `--n` (`scaling`).
    pub scaling: Option<String>,
    pub scaling_n: Option<usize>,
    /// `--external <dir>`.
    pub external: Option<PathBuf>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            compiler: None,
            baseline: None,
            record: false,
            compare_with: None,
            filter: None,
            runs: 5,
            thresholds: ThresholdOverrides::default(),
            mem_limit_mib: process::DEFAULT_MEM_LIMIT_MIB,
            verbose: false,
            paths: Vec::new(),
            scaling: None,
            scaling_n: None,
            external: None,
        }
    }
}

fn parse_pct(flag: &str, v: &str) -> Result<Option<f64>, String> {
    compare::parse_pct(v)
        .map(Some)
        .ok_or_else(|| format!("bad {flag} `{v}` (a non-negative percentage)"))
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
            "--baseline" => o.baseline = Some(PathBuf::from(value("--baseline")?)),
            "--compare-with" => o.compare_with = Some(PathBuf::from(value("--compare-with")?)),
            "--filter" => o.filter = Some(value("--filter")?),
            "--scaling" => o.scaling = Some(value("--scaling")?),
            "--external" => o.external = Some(PathBuf::from(value("--external")?)),
            "--n" => {
                let v = value("--n")?;
                o.scaling_n = Some(
                    v.parse().ok().filter(|&n| n > 0).ok_or_else(|| format!("bad --n `{v}`"))?,
                );
            }
            "--runs" => {
                let v = value("--runs")?;
                o.runs =
                    v.parse().ok().filter(|&n| n > 0).ok_or_else(|| format!("bad --runs `{v}`"))?;
            }
            "--mem-limit" => {
                let v = value("--mem-limit")?;
                o.mem_limit_mib =
                    v.parse().map_err(|_| format!("bad --mem-limit `{v}` (MiB; 0 disables)"))?;
            }
            f @ ("--threshold-time" | "--threshold-mem" | "--threshold-steps"
            | "--threshold-bytes") => {
                let slot = match f {
                    "--threshold-time" => &mut o.thresholds.time_pct,
                    "--threshold-mem" => &mut o.thresholds.mem_pct,
                    "--threshold-steps" => &mut o.thresholds.steps_pct,
                    _ => &mut o.thresholds.bytes_pct,
                };
                *slot = parse_pct(f, &value(f)?)?;
            }
            "--record" | "-v" | "--verbose" if inline.is_some() => {
                return Err(format!("`{flag}` takes no value"));
            }
            "--record" => o.record = true,
            "-v" | "--verbose" => o.verbose = true,
            f if f.starts_with('-') => return Err(format!("unknown option `{f}`")),
            _ => o.paths.push(PathBuf::from(arg)),
        }
    }
    if o.scaling_n.is_some() && o.scaling.is_none() {
        return Err("`--n` needs `--scaling`".into());
    }
    if o.scaling.is_some() && o.external.is_some() {
        return Err("`--scaling` and `--external` are separate runs".into());
    }
    if (o.scaling.is_some() || o.external.is_some())
        && (o.record || o.compare_with.is_some() || !o.paths.is_empty())
    {
        return Err("`--scaling`/`--external` take no paths, `--record` or `--compare-with`".into());
    }
    Ok(Some(o))
}

/// Where the harness reads and writes. Tests point it at fixtures and scratch.
#[derive(Clone, Debug)]
pub struct Layout {
    /// Compilers run with this working directory; entry paths are relative to it.
    pub root: PathBuf,
    /// The bench root: `<group>/<name>.cpp` entries.
    pub bench: PathBuf,
    pub baselines: PathBuf,
    pub results: PathBuf,
    pub perf_md: PathBuf,
    /// Run-private scratch (RSS probe files); removed at the end.
    pub scratch: PathBuf,
}

impl Layout {
    fn standard(root: &Path, target: &Path, baseline: Option<&Path>) -> Self {
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_nanos());
        let bench = root.join("bench");
        Self {
            root: root.to_path_buf(),
            baselines: baseline.map_or_else(
                || bench.join("baselines"),
                |b| if b.is_absolute() { b.to_path_buf() } else { root.join(b) },
            ),
            results: bench.join("results"),
            perf_md: root.join("docs").join("status").join("perf.md"),
            scratch: target.join("xtask-bench").join(format!("run-{}-{nanos}", std::process::id())),
            bench,
        }
    }
}

/// What a run found, for the exit status.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Report {
    pub entries: usize,
    pub failed: usize,
    pub regressed: usize,
    pub improved: usize,
    pub new: usize,
    pub advisory: usize,
    /// kiln failures of `xfail(kiln)` entries (an XPASS counts as failed).
    pub xfail: usize,
    /// Measured entries that are never judged against baselines (the
    /// libc++ slice, `spec/bench.md` "The libcxx-slice entry").
    pub unjudged: usize,
    /// Whether the run fails (exit status 1).
    pub failing: bool,
    /// The results file that was written.
    pub results_file: PathBuf,
}

/// Entry point for `cargo xtask bench`.
pub fn main(args: impl IntoIterator<Item = String>) -> ExitCode {
    let opts = match parse_args(args) {
        Ok(Some(o)) => o,
        Ok(None) => {
            print!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        Err(e) => {
            eprintln!("xtask bench: {e}\n\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    let root = repo_root();
    let result = target_dir(&root).and_then(|target| {
        let layout = Layout::standard(&root, &target, opts.baseline.as_deref());
        let compiler = match &opts.compiler {
            Some(c) => c.clone(),
            None => build_kiln(&root, &target, "bench", true)?,
        };
        if opts.scaling.is_some() || opts.external.is_some() {
            return off_queue(&opts, &root, &target, &compiler);
        }
        let inputs: Vec<PathBuf> = if opts.paths.is_empty() {
            vec![layout.bench.clone()]
        } else {
            opts.paths
                .iter()
                .map(|p| if p.exists() || p.is_absolute() { p.clone() } else { root.join(p) })
                .collect()
        };
        execute(&opts, &layout, &compiler, &inputs)
    });
    match result {
        Ok(r) if r.failing => ExitCode::from(1),
        Ok(_) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("xtask bench: error: {e}");
            ExitCode::from(2)
        }
    }
}

/// `--scaling` and `--external`: no records, no baselines.
fn off_queue(
    opts: &Options,
    root: &Path,
    target: &Path,
    compiler: &Path,
) -> Result<Report, String> {
    let id = identify(compiler)?;
    let scratch = scaling::scratch_dir(target);
    let ok = if opts.scaling.is_some() {
        scaling::run(opts, &scratch, compiler, &id)
    } else {
        external_run(opts, root, &scratch, compiler, id.kind)
    };
    let _ = std::fs::remove_dir_all(&scratch);
    Ok(Report { failing: !ok?, ..Report::default() })
}

/// `cargo xtask bench --external <dir>`: `cargo xtask test --external`'s
/// never-crash run, one case at a time and measured.
fn external_run(
    opts: &Options,
    root: &Path,
    scratch: &Path,
    compiler: &Path,
    kind: CompilerKind,
) -> Result<bool, String> {
    use crate::test::external;
    let given = opts.external.as_deref().unwrap_or(Path::new(""));
    let dir = external::check_outside(given, root)?;
    std::fs::create_dir_all(scratch)
        .map_err(|e| format!("cannot create {}: {e}", scratch.display()))?;
    let probe = RssProbe::detect(scratch);
    let m = external::Measure { probe: &probe, scratch };
    let cap = process::MemLimit::from_mib(opts.mem_limit_mib);
    let (total, summary) = external::run(&dir, compiler, kind, 1, cap, Some(&m))?;
    if opts.verbose {
        for (v, name) in &summary.failures {
            println!("{:<9} {name}", v.label().to_uppercase());
        }
    }
    println!("{}\n{}", summary.line(total), summary.measured_line());
    Ok(!summary.failed())
}

/// Shared state of one invocation.
struct Ctx<'a> {
    layout: &'a Layout,
    probe: RssProbe,
    host: Host,
    runs: usize,
    /// `--mem-limit` (`spec/bench.md` "Resource limits").
    mem_limit: Option<process::MemLimit>,
    verbose: bool,
    /// The libc++ slice's compiles, by compiler path and entry id.
    slice: SliceCompiles,
    /// Tests in the slice's list (the run may select fewer).
    slice_listed: usize,
}

type SliceCompiles =
    BTreeMap<PathBuf, BTreeMap<String, Result<crate::libcxx::SyntaxCompile, String>>>;

/// A compiler taking part in the run.
struct Subject {
    path: PathBuf,
    id: CompilerId,
}

impl Subject {
    fn new(path: &Path) -> Result<Self, String> {
        Ok(Self { path: path.to_path_buf(), id: identify(path)? })
    }

    fn kind(&self) -> &'static str {
        self.id.kind.as_str()
    }

    fn run_record(&self) -> CompilerRun {
        CompilerRun {
            compiler: self.path.display().to_string(),
            kind: self.kind().to_owned(),
            version: self.id.version.clone(),
            entries: BTreeMap::new(),
        }
    }
}

/// Measures `inputs` with `compiler` (and `opts.compare_with`), prints the
/// tables and summary to stdout and progress to stderr, and writes the
/// records. `Err` is an infrastructure problem (exit status 2).
pub fn execute(
    opts: &Options,
    layout: &Layout,
    compiler: &Path,
    inputs: &[PathBuf],
) -> Result<Report, String> {
    let start = Instant::now();
    let (with_slice, rest) = slice::split_inputs(&layout.bench, inputs);
    let mut entries = if rest.is_empty() {
        Vec::new()
    } else {
        entry::discover(&layout.root, &layout.bench, &rest, opts.filter.as_deref())?
    };
    let test = Subject::new(compiler)?;
    let oracle = opts.compare_with.as_deref().map(Subject::new).transpose()?;
    if with_slice {
        slice::check_clang_floor(&test.id, "--compiler", &test.path.display().to_string())?;
        if let Some(o) = &oracle {
            slice::check_clang_floor(&o.id, "--compare-with", &o.path.display().to_string())?;
        }
    }

    std::fs::create_dir_all(&layout.scratch)
        .map_err(|e| format!("cannot create {}: {e}", layout.scratch.display()))?;
    let (mut slice_compiles, mut slice_listed) = (SliceCompiles::new(), 0);
    if with_slice {
        let (listed, slice_entries) =
            slice_setup(opts, layout, [Some(&test), oracle.as_ref()], &mut slice_compiles)
                .inspect_err(|_| {
                    let _ = std::fs::remove_dir_all(&layout.scratch);
                })?;
        slice_listed = listed;
        entries.extend(slice_entries);
    }
    if entries.is_empty() {
        let _ = std::fs::remove_dir_all(&layout.scratch);
        return Err("no bench entries selected".into());
    }
    let cx = Ctx {
        layout,
        probe: RssProbe::detect(&layout.scratch),
        host: Host::current(),
        runs: opts.runs,
        mem_limit: process::MemLimit::from_mib(opts.mem_limit_mib),
        verbose: opts.verbose,
        slice: slice_compiles,
        slice_listed,
    };
    let result = measure_and_report(opts, &cx, &entries, &test, oracle.as_ref(), start);
    let _ = std::fs::remove_dir_all(&layout.scratch);
    result
}

/// Reads the libc++ slice's list and builds each compiler's compiles of the
/// selected tests (`crate::libcxx::syntax_compiles`). Returns the number of
/// listed tests and the selected entries.
fn slice_setup(
    opts: &Options,
    layout: &Layout,
    subjects: [Option<&Subject>; 2],
    out: &mut SliceCompiles,
) -> Result<(usize, Vec<Entry>), String> {
    let list = layout.bench.join(slice::GROUP).join(slice::LIST);
    let shown = shown_path(&layout.root, &list);
    let text = std::fs::read_to_string(&list).map_err(|e| format!("{shown}: {e}"))?;
    let rels = slice::parse_list(&text).map_err(|e| format!("{shown}: {e}"))?;
    let listed = rels.len();
    let rels: Vec<String> = rels
        .into_iter()
        .filter(|r| opts.filter.as_deref().is_none_or(|f| slice::id_of(r).contains(f)))
        .collect();
    let mut test_root = None;
    for (i, s) in subjects.into_iter().flatten().enumerate() {
        let scratch = layout.scratch.join(format!("libcxx-{i}"));
        let clang = s.id.kind == CompilerKind::Clang;
        let sc = crate::libcxx::syntax_compiles(
            &layout.root,
            &s.path,
            clang,
            slice::TAG,
            &rels,
            &scratch,
            opts.mem_limit_mib,
        )?;
        let by_id = rels.iter().map(|r| slice::id_of(r)).zip(sc.tests).collect();
        out.insert(s.path.clone(), by_id);
        test_root = Some(sc.test_root);
    }
    let test_root = test_root.unwrap_or_default();
    Ok((listed, slice::entries(&rels, &test_root, None)))
}

fn measure_and_report(
    opts: &Options,
    cx: &Ctx<'_>,
    entries: &[Entry],
    test: &Subject,
    oracle: Option<&Subject>,
    start: Instant,
) -> Result<Report, String> {
    let now = SystemTime::now();
    let stamp = record::timestamp(now);
    let load = load_avg_1m();
    print_header(cx, entries, test, oracle, load);
    let (test_run, oracle_run, specs, loads) = measure_all(cx, entries, test, oracle)?;
    let (mut report, files) =
        judge_all(opts, cx, entries, &specs, test, &test_run, oracle_run.as_ref())?;

    // Results file.
    let rec = RunRecord {
        timestamp: stamp.clone(),
        host: cx.host.clone(),
        load_avg_1m: load.map(|l| round_to(l, 2)),
        rss_source: cx.probe.source_id().to_owned(),
        runs: cx.runs,
        test: test_run,
        oracle: oracle_run,
    };
    report.results_file = write_results(&cx.layout.results, &stamp, &rec)?;
    println!("\nresults: {}", shown_path(&cx.layout.root, &report.results_file));

    if !loads.0.is_empty() {
        println!("libc++ slice load average (1 min): {}", loads.describe());
    }

    if opts.record {
        record_baselines(cx, entries, &rec.test, &stamp, files)?;
    }

    if let (Some(o), Some(orun)) = (oracle, &rec.oracle) {
        let when = (now, stamp.as_str());
        oracle_report(opts, cx, entries, &specs, (test, &rec.test), (o, orun), when, &loads)?;
    }

    report.failing = report.failed > 0 || (report.regressed > 0 && !opts.record);
    println!(
        "\nbench result: {}. {} entries: {} failed, {} regressed{}, {} improved, {} new, \
         {} advisory{}{} ({:.1}s)",
        if report.failing { "FAILED" } else { "ok" },
        report.entries,
        report.failed,
        report.regressed,
        if opts.record && report.regressed > 0 { " (re-recorded)" } else { "" },
        report.improved,
        report.new,
        report.advisory,
        if report.xfail > 0 { format!(", {} xfail", report.xfail) } else { String::new() },
        if report.unjudged > 0 {
            format!(", {} not judged", report.unjudged)
        } else {
            String::new()
        },
        start.elapsed().as_secs_f64()
    );
    Ok(report)
}

fn print_header(
    cx: &Ctx<'_>,
    entries: &[Entry],
    test: &Subject,
    oracle: Option<&Subject>,
    load: Option<f64>,
) {
    println!(
        "bench: {} entries x {} runs with {} ({}){}",
        entries.len(),
        cx.runs,
        test.path.display(),
        test.id.version,
        oracle.map_or_else(String::new, |o| format!(
            ", oracle {} ({})",
            o.path.display(),
            o.id.version
        )),
    );
    println!(
        "host: {}, {} cpus, load {}; {}",
        cx.host.arch,
        cx.host.cpus,
        load.map_or_else(|| "unknown".to_owned(), |l| format!("{l:.1}")),
        cx.probe.describe()
    );
    #[allow(clippy::cast_precision_loss)]
    if load.is_some_and(|l| l > cx.host.cpus as f64) {
        println!(
            "warning: load average exceeds the CPU count; timings are noisy (the regression \
             rule judges the fastest run, which absorbs most of it)"
        );
    }
}

type Specs = BTreeMap<String, BenchSpec>;

/// Measures every entry with the compiler under test and the oracle, in
/// turn, one entry at a time.
fn measure_all(
    cx: &Ctx<'_>,
    entries: &[Entry],
    test: &Subject,
    oracle: Option<&Subject>,
) -> Result<(CompilerRun, Option<CompilerRun>, Specs, slice::Loads), String> {
    let mut test_run = test.run_record();
    let mut oracle_run = oracle.map(Subject::run_record);
    let mut specs = Specs::new();
    let mut loads = slice::Loads::default();
    for (i, e) in entries.iter().enumerate() {
        let source =
            std::fs::read(&e.path).map_err(|err| format!("{}: {err}", e.path.display()))?;
        let in_slice = slice::is_slice(&e.id);
        let spec = if in_slice {
            loads.0.extend(load_avg_1m());
            Ok(slice::spec())
        } else {
            entry::parse_spec(&String::from_utf8_lossy(&source)).map_err(|errs| {
                errs.iter().map(|d| format!("{}: {d}", e.arg)).collect::<Vec<_>>().join("\n")
            })
        };
        let hash = entry::source_hash(&source);
        let progress = format!(
            "[{:>w$}/{}] {}",
            i + 1,
            entries.len(),
            e.id,
            w = entries.len().to_string().len()
        );
        let mut subjects = vec![(test, &mut test_run)];
        if let (Some(o), Some(r)) = (oracle, oracle_run.as_mut()) {
            subjects.push((o, r));
        }
        for (subject, run) in subjects {
            let outcome = match &spec {
                Ok(spec) if in_slice => measure_slice(cx, e, spec, &hash, subject),
                Ok(spec) => measure(cx, e, spec, &hash, subject),
                Err(why) => Outcome::Failed(format!("bad directives:\n{why}")),
            };
            eprintln!("{progress} {}: {}", subject.kind(), describe_outcome(&outcome));
            run.entries.insert(e.id.clone(), outcome);
        }
        if let Ok(spec) = spec {
            specs.insert(e.id.clone(), spec);
        }
    }
    Ok((test_run, oracle_run, specs, loads))
}

/// Compares the measurements with the baselines of the compiler's kind and
/// prints the table and the failures. Returns the loaded baseline files.
/// The baseline file of every judged entry's group.
fn load_baselines(
    cx: &Ctx<'_>,
    entries: &[Entry],
) -> Result<BTreeMap<String, BaselineFile>, String> {
    let mut files = BTreeMap::new();
    for e in entries.iter().filter(|e| !slice::is_slice(&e.id)) {
        if !files.contains_key(&e.group) {
            let path = cx.layout.baselines.join(format!("{}.json", e.group));
            files.insert(e.group.clone(), BaselineFile::load(&path, &e.group)?);
        }
    }
    Ok(files)
}

fn judge_all(
    opts: &Options,
    cx: &Ctx<'_>,
    entries: &[Entry],
    specs: &Specs,
    test: &Subject,
    test_run: &CompilerRun,
    oracle_run: Option<&CompilerRun>,
) -> Result<(Report, BTreeMap<String, BaselineFile>), String> {
    // Compare with the baselines of the compiler's kind.
    let files = load_baselines(cx, entries)?;
    let mut report = Report { entries: entries.len(), ..Report::default() };
    let mut rows = Vec::new();
    let mut failures = String::new();
    for e in entries {
        let judged = !slice::is_slice(&e.id);
        let base = files
            .get(&e.group)
            .and_then(|f| f.entries.get(&e.name))
            .and_then(|k| k.get(test.kind()));
        let xfail = specs
            .get(&e.id)
            .and_then(|s| s.xfail_kiln.as_deref())
            .filter(|_| test.id.kind == CompilerKind::Kiln);
        match (&test_run.entries[&e.id], xfail) {
            (Outcome::Failed(_), Some(reason)) => {
                report.xfail += 1;
                let mut r = row(e, specs.get(&e.id), base, None, &Verdict::Ok);
                if let Some(status) = r.last_mut() {
                    *status = format!("xfail ({reason})");
                }
                rows.push(r);
            }
            (Outcome::Measured(m), Some(reason)) => {
                report.failed += 1;
                let _ = writeln!(
                    failures,
                    "{} ({}): XPASS: compiled although `xfail(kiln): {reason}`; remove the marker",
                    e.id,
                    test.kind()
                );
                rows.push(row(e, specs.get(&e.id), base, Some(m), &Verdict::Ok));
            }
            (Outcome::Failed(why), None) => {
                report.failed += 1;
                let _ = writeln!(failures, "{} ({}): {why}", e.id, test.kind());
                rows.push(row(e, specs.get(&e.id), base, None, &Verdict::Ok));
            }
            (Outcome::Measured(m), None) if !judged => {
                report.unjudged += 1;
                let mut r = row(e, specs.get(&e.id), None, Some(m), &Verdict::Ok);
                if let Some(status) = r.last_mut() {
                    *status = status.replacen("ok", "not judged", 1);
                }
                rows.push(r);
            }
            (Outcome::Measured(m), None) => {
                let mut t = Thresholds::default();
                if let Some(spec) = specs.get(&e.id) {
                    t = t.with(&spec.thresholds);
                }
                let v = compare::judge(base, m, t.with(&opts.thresholds));
                match &v {
                    Verdict::New => report.new += 1,
                    Verdict::Ok => {}
                    Verdict::Improved => report.improved += 1,
                    Verdict::Regressed { .. } => report.regressed += 1,
                    Verdict::Advisory(_) => report.advisory += 1,
                }
                rows.push(row(e, specs.get(&e.id), base, Some(m), &v));
            }
        }
    }
    let mut header =
        vec!["entry", "mode", "base ms", "ms", "time", "spread", "base MB", "MB", "mem", "status"];
    // Constexpr and instantiation counters (kiln only): columns only when
    // some entry has them.
    let has = |f: fn(&Measurement) -> bool| {
        test_run.entries.values().any(|o| o.measurement().is_some_and(f))
    };
    let (counters, inst) = (has(|m| m.oven.is_some()), has(|m| m.inst.is_some()));
    if inst {
        header.splice(9..9, ["inst", "inst±", "inst KiB", "KiB±"]);
    } else {
        for r in &mut rows {
            r.drain(13..17);
        }
    }
    if counters {
        header.splice(9..9, ["steps", "steps±", "ev bytes", "bytes±"]);
    } else {
        for r in &mut rows {
            r.drain(9..13);
        }
    }
    println!();
    print!("{}", compare::render_table(&header, &rows));
    if let Some(r) = oracle_run {
        for (id, o) in &r.entries {
            if let Outcome::Failed(why) = o {
                report.failed += 1;
                let _ = writeln!(failures, "{id} (oracle {}): {why}", r.kind);
            }
        }
    }
    if !failures.is_empty() {
        println!("\nfailures:\n{}", indent(&failures));
    }
    Ok((report, files))
}

/// Prints the ratio table (and the libc++ slice's) and, with `--record`,
/// merges them into `perf.md`.
#[allow(clippy::too_many_arguments)]
fn oracle_report(
    opts: &Options,
    cx: &Ctx<'_>,
    entries: &[Entry],
    specs: &Specs,
    (test, test_run): (&Subject, &CompilerRun),
    (o, orun): (&Subject, &CompilerRun),
    (now, stamp): (SystemTime, &str),
    loads: &slice::Loads,
) -> Result<(), String> {
    let (slice_rows, ratio_rows): (Vec<RatioRow>, Vec<RatioRow>) = entries
        .iter()
        .map(|e| RatioRow {
            id: e.id.clone(),
            mode: specs.get(&e.id).map_or("-", |s| s.mode.as_str()).to_owned(),
            test: test_run.entries[&e.id].measurement().cloned(),
            oracle: orun.entries[&e.id].measurement().cloned(),
        })
        .partition(|r| slice::is_slice(&r.id));
    let (tl, ol) = compare::ratio_labels(test.kind(), o.kind());
    if !ratio_rows.is_empty() {
        println!();
        print!("{}", compare::render_ratio_table(&ratio_rows, &tl, &ol));
    }
    if !slice_rows.is_empty() {
        println!();
        print!("{}", slice::render_text(&slice_rows, &tl, &ol, loads));
    }
    if !opts.record {
        return Ok(());
    }
    let page = &cx.layout.perf_md;
    let shown = shown_path(&cx.layout.root, page);
    let old_text = std::fs::read_to_string(page).unwrap_or_else(|e| {
        println!("warning: cannot read {shown} ({e}); writing it from this run only");
        String::new()
    });
    let old = perf_md::split(&old_text);
    let tpath = test.path.display().to_string();
    let opath = o.path.display().to_string();
    let when = record::human_time(now);
    let host = format!("{}, {} CPUs", cx.host.arch, cx.host.cpus);
    // The main table: the page's other rows are kept (spec/bench.md
    // "Recording"); one that cannot be read back is rewritten from this run
    // alone. A run of the slice alone keeps it verbatim.
    let mut what = Vec::new();
    let main = if ratio_rows.is_empty() && !old.main.trim().is_empty() {
        old.main.to_owned()
    } else {
        let rows = perf_md::parse(old.main, &tl, &ol).unwrap_or_else(|why| {
            if !old_text.is_empty() {
                println!(
                    "warning: {shown} cannot be merged into ({why}); writing it from this run only"
                );
            }
            Vec::new()
        });
        let corpus: BTreeSet<String> = entry::discover(
            &cx.layout.root,
            &cx.layout.bench,
            std::slice::from_ref(&cx.layout.bench),
            None,
        )?
        .into_iter()
        .map(|e| e.id)
        .collect();
        let full = corpus.iter().all(|id| test_run.entries.contains_key(id));
        let merged = perf_md::merge(
            rows,
            &ratio_rows,
            &corpus,
            full,
            &perf_md::PerfContext {
                when: &when,
                stamp,
                test: (&tpath, test.kind(), &test.id.version),
                oracle: (&opath, o.kind(), &o.id.version),
                host: host.clone(),
                runs: cx.runs,
                rss: cx.probe.source_id(),
            },
        );
        what.push(format!(
            "{} row(s) recorded, {} kept from earlier recordings ({} stale), {} stale dropped",
            merged.recorded, merged.kept, merged.stale, merged.dropped
        ));
        merged.text
    };
    // The slice's section is rewritten by a run that measured it and kept
    // otherwise; so is the hand-written section, always.
    let slice_section = if slice_rows.is_empty() {
        old.slice.map(str::to_owned)
    } else {
        what.push(format!("the libc++ slice section ({} tests)", slice_rows.len()));
        Some(slice::render_section(
            &slice_rows,
            &slice::SectionContext {
                when: &when,
                stamp,
                test: (&tpath, test.kind(), &test.id.version),
                oracle: (&opath, o.kind(), &o.id.version),
                host,
                runs: cx.runs,
                rss: cx.probe.source_id(),
                loads,
                listed: cx.slice_listed,
            },
        ))
    };
    write_file(page, &perf_md::assemble(&main, slice_section.as_deref(), old.kept))?;
    println!("wrote {shown}: {}", what.join("; "));
    Ok(())
}

/// Why a measured run failed: how it ended, under the memory cap or not,
/// and the first lines of its stderr.
fn describe_failure(out: &process::Output, cap: Option<process::MemLimit>) -> String {
    let stderr = out.stderr_str();
    let head: Vec<&str> = stderr.lines().take(5).collect();
    let mut why = out.describe_end();
    if let Some(c) = cap.filter(|_| out.out_of_memory()) {
        why = format!("killed (memory limit {} MiB address space): {why}", c.mib);
    }
    if !head.is_empty() {
        let _ = write!(why, "\n{}", head.join("\n"));
    }
    why
}

/// `mode: run`: builds the entry once, unmeasured, with `cmd` (the compiler
/// and its flags) and returns the command that runs the program.
fn build_program(
    cx: &Ctx<'_>,
    e: &Entry,
    s: &Subject,
    mut cmd: Command,
) -> Result<Command, String> {
    let exe = cx.layout.scratch.join("bench-run.out");
    let _ = std::fs::remove_file(&exe);
    cmd.arg("-o").arg(&exe).arg(&e.arg).current_dir(&cx.layout.root);
    if cx.verbose {
        eprintln!("  $ {}", Shown(&cmd));
    }
    match cmd.output() {
        Ok(o) if o.status.success() => Ok(Command::new(&exe)),
        Ok(o) => {
            let stderr = String::from_utf8_lossy(&o.stderr);
            let head: Vec<&str> = stderr.lines().take(5).collect();
            Err(format!("build failed: {}\n{}", o.status, head.join("\n")))
        }
        Err(err) => Err(format!("cannot run {}: {err}", s.path.display())),
    }
}

/// Runs one entry `cx.runs` times with one compiler.
fn measure(cx: &Ctx<'_>, e: &Entry, spec: &BenchSpec, hash: &str, s: &Subject) -> Outcome {
    let mut flags = vec![format!("-std={}", spec.std)];
    flags.extend(spec.mode.flags().iter().map(|f| (*f).to_owned()));
    flags.extend(spec.args.iter().cloned());
    if s.id.kind == CompilerKind::Kiln
        && e.group == "reflect"
        && let Some(shape) = crate::gen_reflect::shape(&e.name)
    {
        flags.extend(crate::gen_reflect::kiln_flags(shape));
    }
    let run = spec.mode == entry::Mode::Run;
    let mut cmd = Command::new(&s.path);
    cmd.args(&flags);
    if run {
        match build_program(cx, e, s, cmd) {
            Ok(c) => cmd = c,
            Err(why) => return Outcome::Failed(why),
        }
    }
    // kiln reports its constexpr counters; the flag is not part of the
    // recorded `flags`, so it never makes a baseline advisory.
    let report = (s.id.kind == CompilerKind::Kiln
        && !matches!(spec.mode, entry::Mode::Pp | entry::Mode::Run))
    .then(|| cx.layout.scratch.join("constexpr-report.jsonl"));
    if let Some(r) = &report {
        let mut flag = std::ffi::OsString::from(oven_report::FLAG);
        flag.push(r);
        cmd.arg(flag);
    }
    // ... and for `frontend` entries its instantiation totals (the `inst.*`
    // metrics, spec/sema-templates.md §11.4), likewise not in `flags`.
    let inst_report = report.is_some() && e.group == INST_GROUP;
    if inst_report {
        cmd.arg(INST_FLAG);
    }
    if !run {
        cmd.arg(&e.arg);
    }
    cmd.current_dir(&cx.layout.root);
    if cx.verbose {
        eprintln!("  $ {}", Shown(&cmd));
    }
    // kiln reserves its compiling thread's stack up front; the cap is on top.
    let cap = cx.mem_limit.map(|l| {
        if s.id.kind == CompilerKind::Kiln && !run {
            l.plus(process::kiln_stack_reservation_mib(&flags))
        } else {
            l
        }
    });
    let counters = Reports { constexpr: report.as_deref(), inst: inst_report };
    sample(cx, e, spec, s, &cmd, cap, counters, (flags.join(" "), hash))
}

/// Which counter reports a measured command writes.
#[derive(Clone, Copy)]
struct Reports<'a> {
    /// kiln's constexpr report file.
    constexpr: Option<&'a Path>,
    /// kiln's instantiation totals on stderr.
    inst: bool,
}

/// A libc++ slice entry: the compile `crate::libcxx::syntax_compiles` built
/// for this compiler, without counter reports (the runner passes none).
fn measure_slice(cx: &Ctx<'_>, e: &Entry, spec: &BenchSpec, hash: &str, s: &Subject) -> Outcome {
    let c = match cx.slice.get(&s.path).and_then(|m| m.get(&e.id)) {
        Some(Ok(c)) => c,
        Some(Err(why)) => return Outcome::Failed(format!("not a syntax-only compile: {why}")),
        None => return Outcome::Failed("no libc++ compile was built for this entry".into()),
    };
    let mut cmd = Command::new(&s.path);
    cmd.args(&c.args).arg(&c.path).current_dir(&cx.layout.root);
    for (k, v) in &c.env {
        cmd.env(k, v);
    }
    if cx.verbose {
        eprintln!("  $ {}", Shown(&cmd));
    }
    let cap = cx.mem_limit.map(|l| {
        if s.id.kind == CompilerKind::Kiln {
            l.plus(process::kiln_stack_reservation_mib(&c.args))
        } else {
            l
        }
    });
    let flags = slice::shown_flags(&c.args, &cx.layout.root);
    let none = Reports { constexpr: None, inst: false };
    sample(cx, e, spec, s, &cmd, cap, none, (flags, hash))
}

/// Why kiln's fastest run `min_ms` (of `runs`) breaks the entry's `budget-time:`, if
/// it does; other compilers are not held to the budget (spec/bench.md
/// "Directives"). The fastest run, as in the time-regression rule: load
/// only ever adds time, so a budget breach means every run was over.
fn over_time_budget(spec: &BenchSpec, s: &Subject, min_ms: f64, runs: usize) -> Option<String> {
    let budget = spec.budget_time_ms.filter(|_| s.id.kind == CompilerKind::Kiln)?;
    (min_ms > budget).then(|| {
        format!(
            "over the time budget: the fastest of {runs} runs took {min_ms:.1} ms, \
             `budget-time: {budget}` allows {budget} ms"
        )
    })
}

/// Runs `cmd` `cx.runs` times and summarises the runs; `flags` is the
/// recorded flag string and `hash` the entry's source hash.
#[allow(clippy::too_many_arguments)]
fn sample(
    cx: &Ctx<'_>,
    e: &Entry,
    spec: &BenchSpec,
    s: &Subject,
    cmd: &Command,
    cap: Option<process::MemLimit>,
    reports: Reports<'_>,
    (flags, hash): (String, &str),
) -> Outcome {
    let (report, inst_report) = (reports.constexpr, reports.inst);
    let rss_file = cx.layout.scratch.join("rss.txt");
    let mut walls = Vec::with_capacity(cx.runs);
    let mut rss = Vec::with_capacity(cx.runs);
    let mut oven: Option<Counters> = None;
    let mut inst: Option<InstCounters> = None;
    for run in 1..=cx.runs {
        if let Some(r) = report {
            let _ = std::fs::remove_file(r);
        }
        let m = match process::run_measured(cmd, &cx.probe, &rss_file, RUN_TIMEOUT, cap) {
            Ok(m) => m,
            Err(err) => return Outcome::Failed(format!("cannot run {}: {err}", s.path.display())),
        };
        let out = m.as_output();
        if !out.success() {
            return Outcome::Failed(describe_failure(&out, cap));
        }
        walls.push(m.wall.as_secs_f64() * 1000.0);
        if let Some(k) = m.max_rss_kb {
            rss.push(k);
        }
        if let Some(r) = report {
            let got = read_counters(r, e, spec);
            if let Err(why) = same_every_run(&mut oven, got, run, "constexpr", Counters::describe) {
                return Outcome::Failed(why);
            }
        }
        if inst_report {
            let got = InstCounters::parse_report(&out.stderr_str());
            let what = "instantiation";
            if let Err(why) = same_every_run(&mut inst, got, run, what, |c| c.describe()) {
                return Outcome::Failed(why);
            }
        }
    }
    let Some(w) = Summary::of(&walls) else { return Outcome::Failed("no runs".into()) };
    if let Some(why) = over_time_budget(spec, s, w.min, walls.len()) {
        return Outcome::Failed(why);
    }
    // Peak RSS only counts when every run reported it.
    #[allow(clippy::cast_precision_loss)]
    let max_rss_kb = if rss.len() == walls.len() {
        Summary::of(&rss.iter().map(|&k| k as f64).collect::<Vec<_>>())
            .map(|s| s.median.round() as u64)
    } else {
        None
    };
    Outcome::Measured(Measurement {
        mode: spec.mode,
        flags,
        source_hash: hash.to_owned(),
        runs: cx.runs,
        wall_ms_median: round_to(w.median, 3),
        wall_ms_min: round_to(w.min, 3),
        wall_ms_max: round_to(w.max, 3),
        spread_pct: round_to(w.spread_pct(), 2),
        max_rss_kb,
        oven,
        oven_missing: Vec::new(),
        inst,
        samples_wall_ms: walls.iter().map(|&x| round_to(x, 3)).collect(),
        samples_rss_kb: rss,
        compiler: s.path.display().to_string(),
        compiler_version: s.id.version.clone(),
        host: cx.host.clone(),
    })
}

/// Records run `run`'s counters in `slot`, failing when reading them failed
/// or when they differ from run 1's: the counters are deterministic.
fn same_every_run<T: Copy + PartialEq>(
    slot: &mut Option<T>,
    got: Result<T, String>,
    run: usize,
    what: &str,
    describe: impl Fn(&T) -> String,
) -> Result<(), String> {
    let c = got.map_err(|why| format!("run {run}: {why}"))?;
    match slot {
        Some(first) if *first != c => Err(format!(
            "nondeterministic {what} counters: run 1 {}, run {run} {}",
            describe(first),
            describe(&c)
        )),
        _ => {
            *slot = Some(c);
            Ok(())
        }
    }
}

/// Reads and checks one run's constexpr report: the total line, with
/// `wall_us` dropped (it is noise, not a counter), after the entry's `oven:`
/// budgets pass.
fn read_counters(path: &Path, e: &Entry, spec: &BenchSpec) -> Result<Counters, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|err| format!("the compiler wrote no readable constexpr report ({err})"))?;
    let report = oven_report::Report::parse(&text)
        .map_err(|err| format!("malformed constexpr report: {err}"))?;
    let in_entry =
        |f: &str| f == e.arg || std::fs::canonicalize(Path::new(f)).is_ok_and(|p| p == e.path);
    oven_report::check_budgets(&report, &spec.oven, &in_entry)
        .map_err(|err| err.trim_end().to_owned())?;
    // `memo_hits`/`refl_bytes`/`splice_fast` are budget-only counters: the bench does not record them.
    Ok(Counters { wall_us: 0, memo_hits: 0, refl_bytes: 0, splice_fast: 0, ..report.total })
}

fn describe_outcome(o: &Outcome) -> String {
    match o {
        Outcome::Failed(why) => format!("FAILED ({})", why.lines().next().unwrap_or_default()),
        Outcome::Measured(m) => {
            let mut s = format!(
                "{} ms (min {}, spread {:.1}%), {} MB",
                compare::ms(m.wall_ms_median),
                compare::ms(m.wall_ms_min),
                m.spread_pct,
                compare::mb(m.max_rss_kb)
            );
            if m.spread_pct > NOISY_SPREAD_PCT {
                let _ = write!(s, "  warning: noisy (spread > {NOISY_SPREAD_PCT}%; try --runs)");
            }
            s
        }
    }
}

fn row(
    entry: &Entry,
    spec: Option<&BenchSpec>,
    base: Option<&Baseline>,
    cur: Option<&Measurement>,
    verdict: &Verdict,
) -> Vec<String> {
    let b = base.map(|b| &b.measurement);
    #[allow(clippy::cast_precision_loss)]
    let mem_delta = match (b.and_then(|b| b.max_rss_kb), cur.and_then(|c| c.max_rss_kb)) {
        (Some(base_kb), Some(cur_kb)) => compare::delta_pct(base_kb as f64, cur_kb as f64),
        _ => None,
    };
    let mut status = match cur {
        None => "FAILED".to_owned(),
        Some(_) => verdict.label(),
    };
    if cur.is_some_and(|c| c.spread_pct > NOISY_SPREAD_PCT) {
        status.push_str(" (noisy)");
    }
    let dash = || "-".to_owned();
    let (bo, co) = (b.and_then(|b| b.oven), cur.and_then(|c| c.oven));
    #[allow(clippy::cast_precision_loss)]
    let counter_delta = |f: fn(&Counters) -> u64| match (&bo, &co) {
        (Some(x), Some(y)) => compare::delta_pct(f(x) as f64, f(y) as f64),
        _ => None,
    };
    let (bi, ci) = (b.and_then(|b| b.inst), cur.and_then(|c| c.inst));
    #[allow(clippy::cast_precision_loss)]
    let inst_delta = |f: fn(&InstCounters) -> u64| match (&bi, &ci) {
        (Some(x), Some(y)) => compare::delta_pct(f(x) as f64, f(y) as f64),
        _ => None,
    };
    #[allow(clippy::cast_precision_loss)]
    let kib = |n: u64| format!("{:.1}", n as f64 / 1024.0);
    vec![
        entry.id.clone(),
        spec.map_or_else(dash, |s| s.mode.as_str().to_owned()),
        b.map_or_else(dash, |b| compare::ms(b.wall_ms_median)),
        cur.map_or_else(dash, |c| compare::ms(c.wall_ms_median)),
        compare::pct(
            b.zip(cur).and_then(|(b, c)| compare::delta_pct(b.wall_ms_median, c.wall_ms_median)),
        ),
        cur.map_or_else(dash, |c| format!("{:.1}%", c.spread_pct)),
        b.map_or_else(dash, |b| compare::mb(b.max_rss_kb)),
        cur.map_or_else(dash, |c| compare::mb(c.max_rss_kb)),
        compare::pct(mem_delta),
        co.map_or_else(dash, |c| c.steps.to_string()),
        compare::pct(counter_delta(|c| c.steps)),
        co.map_or_else(dash, |c| c.bytes.to_string()),
        compare::pct(counter_delta(|c| c.bytes)),
        ci.map_or_else(dash, |i| i.count.to_string()),
        compare::pct(inst_delta(|i| i.count)),
        ci.map_or_else(dash, |i| kib(i.bytes)),
        compare::pct(inst_delta(|i| i.bytes)),
        status,
    ]
}

/// Merges the measured entries into `bench/baselines/<group>.json`, keeping
/// baselines of entries and compiler kinds that did not run.
fn record_baselines(
    cx: &Ctx<'_>,
    entries: &[Entry],
    run: &CompilerRun,
    stamp: &str,
    mut files: BTreeMap<String, BaselineFile>,
) -> Result<(), String> {
    let mut touched = BTreeMap::new();
    // The libc++ slice is record-only: no baselines.
    for e in entries.iter().filter(|e| !slice::is_slice(&e.id)) {
        if let Outcome::Measured(m) = &run.entries[&e.id] {
            let mut m = m.clone();
            m.samples_wall_ms.clear();
            m.samples_rss_kb.clear();
            let file = files.get_mut(&e.group).expect("loaded above");
            file.entries
                .entry(e.name.clone())
                .or_default()
                .insert(run.kind.clone(), Baseline { measurement: m, recorded: stamp.to_owned() });
            *touched.entry(e.group.clone()).or_insert(0usize) += 1;
        }
    }
    for (group, n) in touched {
        let path = cx.layout.baselines.join(format!("{group}.json"));
        write_file(&path, &files[&group].to_json(&group).to_pretty())?;
        println!("recorded {n} {} baseline(s) in {}", run.kind, shown_path(&cx.layout.root, &path));
    }
    Ok(())
}

/// Writes `<dir>/<stamp>.json`, adding `-1`, `-2`, ... if the name is taken.
fn write_results(dir: &Path, stamp: &str, rec: &RunRecord) -> Result<PathBuf, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    let text = rec.to_json().to_pretty();
    for n in 0.. {
        let name = if n == 0 { format!("{stamp}.json") } else { format!("{stamp}-{n}.json") };
        let path = dir.join(name);
        match std::fs::OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(mut f) => {
                use std::io::Write as _;
                f.write_all(text.as_bytes()).map_err(|e| format!("{}: {e}", path.display()))?;
                return Ok(path);
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(format!("{}: {e}", path.display())),
        }
    }
    unreachable!("an unbounded search finds a free name")
}

fn write_file(path: &Path, text: &str) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)
            .map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    }
    std::fs::write(path, text).map_err(|e| format!("{}: {e}", path.display()))
}

fn shown_path(root: &Path, p: &Path) -> String {
    p.strip_prefix(root).unwrap_or(p).display().to_string()
}

fn indent(s: &str) -> String {
    s.lines().map(|l| format!("  {l}")).collect::<Vec<_>>().join("\n")
}

/// The 1-minute load average, where the OS exposes it.
fn load_avg_1m() -> Option<f64> {
    std::fs::read_to_string("/proc/loadavg").ok()?.split_whitespace().next()?.parse().ok()
}
