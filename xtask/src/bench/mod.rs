//! `cargo xtask bench`: the performance harness.
//!
//! The contract (CLI, directives, records, regression policy) is
//! `spec/bench.md`; the policy background is `docs/VALIDATION.md`.

mod compare;
mod entry;
mod record;
mod stats;
#[cfg(test)]
mod tests;

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use compare::{RatioRow, Thresholds, Verdict};
use entry::{BenchSpec, Entry};
use record::{Baseline, BaselineFile, CompilerRun, Host, Measurement, Outcome, RunRecord};
use stats::{Summary, round_to};

use crate::util::process::{self, RssProbe, Shown};
use crate::util::{CompilerId, build_kiln, identify, repo_root, target_dir};

pub const USAGE: &str = "\
usage: cargo xtask bench [options] [paths...]

Measures compile time and peak memory of the bench/ entries (or the given
files/directories under bench/) and compares them with bench/baselines.
See spec/bench.md.

options:
  --compiler <path>        compiler to measure (default: build and use a release kiln)
  --baseline <dir>         baseline directory (default: bench/baselines)
  --record                 write the measurements as the new baselines; with
                           --compare-with, also write docs/status/perf.md
  --compare-with <path>    also measure this compiler (e.g. clang++-19) and print
                           the time and memory ratios
  --filter <substr>        only entries whose id (<group>/<name>) contains <substr>
  --runs <N>               runs per entry (default: 5; the fastest and slowest are
                           dropped when N >= 3)
  --threshold-time <PCT>   allowed wall-clock regression in percent (default: 5)
  --threshold-mem <PCT>    allowed peak-RSS regression in percent (default: 2)
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

/// Parsed command line.
#[derive(Clone, Debug, PartialEq)]
pub struct Options {
    pub compiler: Option<PathBuf>,
    pub baseline: Option<PathBuf>,
    pub record: bool,
    pub compare_with: Option<PathBuf>,
    pub filter: Option<String>,
    pub runs: usize,
    pub thresholds: Thresholds,
    pub verbose: bool,
    pub paths: Vec<PathBuf>,
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
            thresholds: Thresholds::default(),
            verbose: false,
            paths: Vec::new(),
        }
    }
}

fn parse_pct(flag: &str, v: &str) -> Result<f64, String> {
    v.parse()
        .ok()
        .filter(|p: &f64| p.is_finite() && *p >= 0.0)
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
            "--runs" => {
                let v = value("--runs")?;
                o.runs =
                    v.parse().ok().filter(|&n| n > 0).ok_or_else(|| format!("bad --runs `{v}`"))?;
            }
            "--threshold-time" => {
                o.thresholds.time_pct = parse_pct("--threshold-time", &value("--threshold-time")?)?;
            }
            "--threshold-mem" => {
                o.thresholds.mem_pct = parse_pct("--threshold-mem", &value("--threshold-mem")?)?;
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

/// Shared state of one invocation.
struct Ctx<'a> {
    layout: &'a Layout,
    probe: RssProbe,
    host: Host,
    runs: usize,
    verbose: bool,
}

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
    let entries = entry::discover(&layout.root, &layout.bench, inputs, opts.filter.as_deref())?;
    if entries.is_empty() {
        return Err("no bench entries selected".into());
    }
    let test = Subject::new(compiler)?;
    let oracle = opts.compare_with.as_deref().map(Subject::new).transpose()?;

    std::fs::create_dir_all(&layout.scratch)
        .map_err(|e| format!("cannot create {}: {e}", layout.scratch.display()))?;
    let cx = Ctx {
        layout,
        probe: RssProbe::detect(&layout.scratch),
        host: Host::current(),
        runs: opts.runs,
        verbose: opts.verbose,
    };
    let result = measure_and_report(opts, &cx, &entries, &test, oracle.as_ref(), start);
    let _ = std::fs::remove_dir_all(&layout.scratch);
    result
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
    let (test_run, oracle_run, specs) = measure_all(cx, entries, test, oracle)?;
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

    if opts.record {
        record_baselines(cx, entries, &rec.test, &stamp, files)?;
    }

    if let (Some(o), Some(orun)) = (oracle, &rec.oracle) {
        oracle_report(opts, cx, entries, &specs, (test, &rec.test), (o, orun), now)?;
    }

    report.failing = report.failed > 0 || (report.regressed > 0 && !opts.record);
    println!(
        "\nbench result: {}. {} entries: {} failed, {} regressed{}, {} improved, {} new, \
         {} advisory ({:.1}s)",
        if report.failing { "FAILED" } else { "ok" },
        report.entries,
        report.failed,
        report.regressed,
        if opts.record && report.regressed > 0 { " (re-recorded)" } else { "" },
        report.improved,
        report.new,
        report.advisory,
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
) -> Result<(CompilerRun, Option<CompilerRun>, Specs), String> {
    let mut test_run = test.run_record();
    let mut oracle_run = oracle.map(Subject::run_record);
    let mut specs = Specs::new();
    for (i, e) in entries.iter().enumerate() {
        let source =
            std::fs::read(&e.path).map_err(|err| format!("{}: {err}", e.path.display()))?;
        let spec = entry::parse_spec(&String::from_utf8_lossy(&source)).map_err(|errs| {
            errs.iter().map(|d| format!("{}: {d}", e.arg)).collect::<Vec<_>>().join("\n")
        });
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
    Ok((test_run, oracle_run, specs))
}

/// Compares the measurements with the baselines of the compiler's kind and
/// prints the table and the failures. Returns the loaded baseline files.
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
    let mut files: BTreeMap<String, BaselineFile> = BTreeMap::new();
    for e in entries {
        if !files.contains_key(&e.group) {
            let f = BaselineFile::load(
                &cx.layout.baselines.join(format!("{}.json", e.group)),
                &e.group,
            )?;
            files.insert(e.group.clone(), f);
        }
    }
    let mut report = Report { entries: entries.len(), ..Report::default() };
    let mut rows = Vec::new();
    let mut failures = String::new();
    for e in entries {
        let base = files[&e.group].entries.get(&e.name).and_then(|k| k.get(test.kind()));
        match &test_run.entries[&e.id] {
            Outcome::Failed(why) => {
                report.failed += 1;
                let _ = writeln!(failures, "{} ({}): {why}", e.id, test.kind());
                rows.push(row(e, specs.get(&e.id), base, None, &Verdict::Ok));
            }
            Outcome::Measured(m) => {
                let v = compare::judge(base, m, opts.thresholds);
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
    let header =
        ["entry", "mode", "base ms", "ms", "time", "spread", "base MB", "MB", "mem", "status"];
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

/// Prints the ratio table and, with `--record`, writes `perf.md`.
fn oracle_report(
    opts: &Options,
    cx: &Ctx<'_>,
    entries: &[Entry],
    specs: &Specs,
    (test, test_run): (&Subject, &CompilerRun),
    (o, orun): (&Subject, &CompilerRun),
    now: SystemTime,
) -> Result<(), String> {
    {
        let ratio_rows: Vec<RatioRow> = entries
            .iter()
            .map(|e| RatioRow {
                id: e.id.clone(),
                mode: specs.get(&e.id).map_or("-", |s| s.mode.as_str()).to_owned(),
                test: test_run.entries[&e.id].measurement().cloned(),
                oracle: orun.entries[&e.id].measurement().cloned(),
            })
            .collect();
        let (tl, ol) = compare::ratio_labels(test.kind(), o.kind());
        println!();
        print!("{}", compare::render_ratio_table(&ratio_rows, &tl, &ol));
        if opts.record {
            let tpath = test.path.display().to_string();
            let opath = o.path.display().to_string();
            let md = compare::render_perf_md(
                &ratio_rows,
                &compare::PerfContext {
                    when: &record::human_time(now),
                    test: (&tpath, test.kind(), &test.id.version),
                    oracle: (&opath, o.kind(), &o.id.version),
                    host: format!("{}, {} CPUs", cx.host.arch, cx.host.cpus),
                    runs: cx.runs,
                    rss: cx.probe.source_id(),
                    filter: opts.filter.as_deref(),
                },
            );
            write_file(&cx.layout.perf_md, &md)?;
            println!("wrote {}", shown_path(&cx.layout.root, &cx.layout.perf_md));
        }
    }
    Ok(())
}

/// Runs one entry `cx.runs` times with one compiler.
fn measure(cx: &Ctx<'_>, e: &Entry, spec: &BenchSpec, hash: &str, s: &Subject) -> Outcome {
    let mut flags = vec![format!("-std={}", spec.std)];
    flags.extend(spec.mode.flags().iter().map(|f| (*f).to_owned()));
    flags.extend(spec.args.iter().cloned());
    let mut cmd = Command::new(&s.path);
    cmd.args(&flags).arg(&e.arg).current_dir(&cx.layout.root);
    if cx.verbose {
        eprintln!("  $ {}", Shown(&cmd));
    }
    let rss_file = cx.layout.scratch.join("rss.txt");
    let mut walls = Vec::with_capacity(cx.runs);
    let mut rss = Vec::with_capacity(cx.runs);
    for _ in 0..cx.runs {
        let m = match process::run_measured(&cmd, &cx.probe, &rss_file, RUN_TIMEOUT) {
            Ok(m) => m,
            Err(err) => return Outcome::Failed(format!("cannot run {}: {err}", s.path.display())),
        };
        let out = m.as_output();
        if !out.success() {
            let stderr = out.stderr_str();
            let head: Vec<&str> = stderr.lines().take(5).collect();
            let mut why = out.describe_end();
            if !head.is_empty() {
                let _ = write!(why, "\n{}", head.join("\n"));
            }
            return Outcome::Failed(why);
        }
        walls.push(m.wall.as_secs_f64() * 1000.0);
        if let Some(k) = m.max_rss_kb {
            rss.push(k);
        }
    }
    let Some(w) = Summary::of(&walls) else { return Outcome::Failed("no runs".into()) };
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
        flags: flags.join(" "),
        source_hash: hash.to_owned(),
        runs: cx.runs,
        wall_ms_median: round_to(w.median, 3),
        wall_ms_min: round_to(w.min, 3),
        wall_ms_max: round_to(w.max, 3),
        spread_pct: round_to(w.spread_pct(), 2),
        max_rss_kb,
        samples_wall_ms: walls.iter().map(|&x| round_to(x, 3)).collect(),
        samples_rss_kb: rss,
        compiler: s.path.display().to_string(),
        compiler_version: s.id.version.clone(),
        host: cx.host.clone(),
    })
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
    for e in entries {
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
