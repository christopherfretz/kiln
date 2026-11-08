//! `cargo xtask clang-neg` — clang's own `-verify` tests run through kiln
//! to measure what kiln wrongly accepts: every `%clang_cc1 -verify` RUN
//! line of `clang/test/{CXX,SemaCXX}` becomes one kiln `-fsyntax-only
//! -verify` case, and the file-level accepts-invalid and line-level
//! missed-error counts go to a JSON report and a Markdown status page.
//! Wording is never compared. Contract: `spec/clang-neg.md`.

mod outcome;
mod report;
mod runline;

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::libcxx::cluster::crash_key;
use crate::libstdcxx::glob_match;
use crate::util::process::{self, MemLimit, RssProbe};
use crate::util::{Profile, build_kiln_with, repo_root, target_dir};
use outcome::Classified;
use runline::{CaseCfg, RunPlan};

const USAGE: &str = "\
usage: cargo xtask clang-neg [options] [<dir-or-file>...]

Runs clang's own `-verify` tests through kiln (-fsyntax-only -verify, every
expected-error made unmatchable) and reports what kiln wrongly accepts: files
where clang expects an error and kiln reports none (ACCEPTS-INVALID), and
expected-error lines with no kiln error (MISSED-ERROR). Wording is never
compared. Paths are relative to <tests>/clang/test (default: CXX SemaCXX).
Informational: the exit status is non-zero only when the tool itself fails.
Contract: spec/clang-neg.md.

  --tests <dir>          an llvm-project checkout holding clang/test (default:
                         .oss/clang-tests; scripts/fetch-clang-tests.sh makes it)
  --profile <p>          the cargo profile to build kiln++ with: debug, harness
                         (default) or release
  --compiler <path>      kiln++ binary (default: cargo build --profile <p> -p kiln_driver)
  --filter <glob>        only tests whose path (relative to clang/test) matches
  --list                 list the planned cases and skips, run nothing
  -j, --jobs <n>         cases in parallel (default 4; an upper bound under a
                         MAKEFLAGS jobserver, see spec/harness.md)
  --timeout <secs>       per-case timeout (default 60)
  --mem-limit <MiB>      address-space cap per case, plus kiln's stack
                         reservation (default 2048; 0: none)
  --json <file>          the JSON report (default: <target>/xtask-clang-neg/report.json)
  --report <file.md>     write the generated region of the status page
  -v, --verbose          print every case's result
";

/// The llvm-project tag the published numbers were measured against.
pub const CLANG_TAG: &str = "llvmorg-22.1.8";

/// The directories run when none is named.
pub const DEFAULT_DIRS: &[&str] = &["CXX", "SemaCXX"];

#[derive(Clone, Debug, PartialEq)]
pub struct Options {
    pub tests: Option<PathBuf>,
    pub paths: Vec<String>,
    pub profile: Profile,
    pub compiler: Option<PathBuf>,
    pub filter: Option<String>,
    pub list: bool,
    pub jobs: usize,
    pub timeout: Duration,
    pub mem_limit_mib: u64,
    pub json: Option<PathBuf>,
    pub report: Option<PathBuf>,
    pub verbose: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            tests: None,
            paths: Vec::new(),
            profile: Profile::Harness,
            compiler: None,
            filter: None,
            list: false,
            jobs: 4,
            timeout: Duration::from_secs(60),
            mem_limit_mib: 2048,
            json: None,
            report: None,
            verbose: false,
        }
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
            "--tests" => o.tests = Some(PathBuf::from(value("--tests")?)),
            "--profile" => o.profile = Profile::parse(&value("--profile")?)?,
            "--compiler" => o.compiler = Some(PathBuf::from(value("--compiler")?)),
            "--filter" => o.filter = Some(value("--filter")?),
            "--json" => o.json = Some(PathBuf::from(value("--json")?)),
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
                o.timeout = Duration::from_secs_f64(secs);
            }
            "--mem-limit" => {
                let v = value("--mem-limit")?;
                o.mem_limit_mib =
                    v.parse().map_err(|_| format!("bad --mem-limit `{v}` (MiB; 0 disables)"))?;
            }
            "--list" | "-v" | "--verbose" if inline.is_some() => {
                return Err(format!("`{flag}` takes no value"));
            }
            "--list" => o.list = true,
            "-v" | "--verbose" => o.verbose = true,
            f if f.starts_with('-') => return Err(format!("unknown argument `{f}`")),
            p => o.paths.push(p.trim_end_matches('/').to_owned()),
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
            eprintln!("xtask clang-neg: {e}\n\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    match execute(&opts) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("xtask clang-neg: {e}");
            ExitCode::from(2)
        }
    }
}

/// The report row a test path belongs to: `CXX/<section>` for the
/// standard-section tree, else its top directory (`SemaCXX`).
pub fn dir_key(rel: &str) -> String {
    let mut parts = rel.split('/');
    let top = parts.next().unwrap_or("");
    match (top, parts.next()) {
        ("CXX", Some(sec)) if rel.matches('/').count() >= 2 => format!("CXX/{sec}"),
        _ => top.to_owned(),
    }
}

/// The `.cpp` tests under `base/<p>` for each selected path, relative to
/// `base`, sorted; `Inputs/` directories hold headers, not tests.
pub fn discover(base: &Path, paths: &[String]) -> Result<Vec<String>, String> {
    fn walk(base: &Path, dir: &Path, out: &mut Vec<String>) -> Result<(), String> {
        let rd =
            std::fs::read_dir(dir).map_err(|e| format!("cannot read {}: {e}", dir.display()))?;
        for e in rd.filter_map(Result::ok) {
            let p = e.path();
            if p.is_dir() {
                if e.file_name() != "Inputs" {
                    walk(base, &p, out)?;
                }
            } else if p.extension().is_some_and(|x| x == "cpp")
                && let Ok(rel) = p.strip_prefix(base)
            {
                out.push(rel.to_string_lossy().replace('\\', "/"));
            }
        }
        Ok(())
    }
    let mut out = Vec::new();
    for p in paths {
        let full = base.join(p);
        if full.is_file() {
            out.push(p.clone());
        } else if full.is_dir() {
            walk(base, &full, &mut out)?;
        } else {
            return Err(format!("{} does not exist", full.display()));
        }
    }
    out.sort();
    out.dedup();
    Ok(out)
}

/// What one test file contributes: its cases (one per RUN configuration,
/// duplicates after flag mapping dropped) and its skips by reason.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FilePlan {
    pub cases: Vec<CaseCfg>,
    pub skips: Vec<(String, usize)>,
    /// Whether any RUN line is a `-verify` run.
    pub verify: bool,
}

/// Plans one file (spec/clang-neg.md "RUN lines").
pub fn plan_file(text: &str, dir: &str) -> FilePlan {
    let mut fp = FilePlan::default();
    let mut seen: BTreeSet<(String, Vec<String>, Vec<String>)> = BTreeSet::new();
    for (i, cmd) in runline::run_commands(text).iter().enumerate() {
        match runline::plan(cmd, i + 1, dir) {
            RunPlan::NotVerify => {}
            RunPlan::Skip { reason, configs } => {
                fp.verify = true;
                fp.skips.push((reason, configs));
            }
            RunPlan::Cases(cs) => {
                fp.verify = true;
                for c in cs {
                    if seen.insert((c.std.clone(), c.prefixes.clone(), c.args.clone())) {
                        fp.cases.push(c);
                    } else {
                        fp.skips.push(("duplicate configuration after flag mapping".into(), 1));
                    }
                }
            }
        }
    }
    if let Some(reason) = runline::requirements(text)
        && !fp.cases.is_empty()
    {
        let n = fp.cases.len();
        fp.cases.clear();
        fp.skips.push((reason, n));
    }
    fp
}

/// How a case ended.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Status {
    /// kiln ran to the end of its `-verify` report.
    Ran(Classified),
    /// kiln panicked or died on a signal; the cluster key.
    Crash(String),
    Timeout,
    Oom,
    /// The kiln driver rejected a mapped flag: a skip, not a verdict.
    FlagRejected(String),
    /// The case could not be run at all.
    ToolError(String),
}

/// One case's result.
#[derive(Clone, Debug)]
pub struct CaseResult {
    pub file: String,
    pub cfg: CaseCfg,
    pub status: Status,
    /// The file declares `<prefix>-no-diagnostics` for the case's prefixes.
    pub no_diagnostics: bool,
    pub wall: Duration,
    pub max_rss_kb: Option<u64>,
}

impl CaseResult {
    pub fn classified(&self) -> Option<&Classified> {
        match &self.status {
            Status::Ran(c) => Some(c),
            _ => None,
        }
    }

    /// kiln reports an error in a file clang expects no diagnostics from.
    pub fn rejects_valid(&self) -> bool {
        self.no_diagnostics
            && self.classified().is_some_and(|c| c.expected.is_empty() && !c.kiln_errors.is_empty())
    }

    fn console_line(&self) -> String {
        let what = match &self.status {
            Status::Ran(c) if c.accepts_invalid() => {
                format!("ACCEPTS-INVALID ({} expected errors)", c.expected.len())
            }
            _ if self.rejects_valid() => "REJECTS-VALID".to_owned(),
            Status::Ran(c) => format!("{}/{} expected errors missed", c.missed.len(), c.expected.len()),
            Status::Crash(k) => format!("CRASH {k}"),
            Status::Timeout => "TIMEOUT".to_owned(),
            Status::Oom => "OOM".to_owned(),
            Status::FlagRejected(r) => format!("skipped: {r}"),
            Status::ToolError(e) => format!("tool error: {e}"),
        };
        format!("{} [{}] {what}", self.file, self.cfg.label())
    }
}

struct Config {
    kiln: PathBuf,
    base: PathBuf,
    scratch: PathBuf,
    timeout: Duration,
    mem: Option<MemLimit>,
    probe: RssProbe,
}

struct Job<'a> {
    file: &'a str,
    text: &'a str,
    cfg: CaseCfg,
}

/// The kiln++ arguments of a case, before the input path; a `%s` flag
/// value (`-include %s`) becomes the test's copy.
pub fn kiln_args(cfg: &CaseCfg, orig_dir: &Path, copy: &Path) -> Vec<String> {
    let mut a = vec![
        "-fsyntax-only".to_owned(),
        "-ferror-limit=0".to_owned(),
        format!("-std={}", cfg.std),
        format!("-verify={}", cfg.prefixes.join(",")),
        "-iquote".to_owned(),
        orig_dir.display().to_string(),
    ];
    a.extend(cfg.args.iter().map(|x| if x == "%s" { copy.display().to_string() } else { x.clone() }));
    a
}

fn run_case(cfg: &Config, job: &Job<'_>, idx: usize) -> CaseResult {
    let start = Instant::now();
    let mut r = CaseResult {
        file: job.file.to_owned(),
        cfg: job.cfg.clone(),
        status: Status::ToolError(String::new()),
        no_diagnostics: outcome::declares_no_diagnostics(job.text, &job.cfg.prefixes),
        wall: Duration::ZERO,
        max_rss_kb: None,
    };
    let src = cfg.base.join(job.file);
    let orig_dir = src.parent().unwrap_or(&cfg.base).to_path_buf();
    let dir = cfg.scratch.join(idx.to_string());
    let name = src.file_name().map_or_else(|| "t.cpp".into(), |n| n.to_string_lossy().into_owned());
    let copy = dir.join(&name);
    if let Err(e) = std::fs::create_dir_all(&dir)
        .and_then(|()| std::fs::write(&copy, outcome::neutralize(job.text)))
    {
        r.status = Status::ToolError(format!("cannot write {}: {e}", copy.display()));
        return r;
    }
    let args = kiln_args(&job.cfg, &orig_dir, &copy);
    let limit = cfg.mem.map(|m| m.plus(process::kiln_stack_reservation_mib(&args)));
    let mut cmd = Command::new(&cfg.kiln);
    cmd.args(&args).arg(&copy).current_dir(&dir);
    r.status = match process::run_measured(&cmd, &cfg.probe, &dir.join("rss.txt"), cfg.timeout, limit)
    {
        Ok(m) => {
            r.wall = m.wall;
            r.max_rss_kb = m.max_rss_kb;
            judge(m.status, &String::from_utf8_lossy(&m.stderr))
        }
        Err(e) => Status::ToolError(format!("cannot run {}: {e}", cfg.kiln.display())),
    };
    if r.wall.is_zero() {
        r.wall = start.elapsed();
    }
    let _ = std::fs::remove_dir_all(&dir);
    r
}

/// Classifies one finished kiln run from its exit status and stderr.
pub fn judge(status: Option<std::process::ExitStatus>, stderr: &str) -> Status {
    let Some(st) = status else { return Status::Timeout };
    if process::out_of_memory(Some(st), stderr) {
        return Status::Oom;
    }
    if let Some((key, _)) = crash_key(stderr) {
        return Status::Crash(key);
    }
    if st.code().is_none_or(|c| c >= 128) {
        return Status::Crash(format!("crash: {st}"));
    }
    if let Some(line) = stderr.lines().find(|l| l.starts_with("kiln++: error: ") || l.starts_with("kiln: error: ")) {
        let msg = line.split_once("error: ").map_or(line, |(_, m)| m);
        return Status::FlagRejected(format!("kiln: {msg}"));
    }
    Status::Ran(outcome::classify(&outcome::parse_report(stderr)))
}

/// The scheduled jobs and the skips, from the selected files.
fn plan_all<'a>(base: &Path, files: &'a [(String, String)]) -> (Vec<Job<'a>>, report::Skips, usize) {
    let mut jobs = Vec::new();
    let mut skips = report::Skips::default();
    let mut verify_files = 0;
    for (rel, text) in files {
        let dir = base.join(rel);
        let dir = dir.parent().unwrap_or(base).display().to_string();
        let fp = plan_file(text, &dir);
        verify_files += usize::from(fp.verify);
        for (reason, n) in fp.skips {
            skips.add(&reason, rel, n);
        }
        jobs.extend(fp.cases.into_iter().map(|cfg| Job { file: rel, text, cfg }));
    }
    (jobs, skips, verify_files)
}

fn execute(opts: &Options) -> Result<(), String> {
    let start = Instant::now();
    let root = repo_root();
    let tests = opts.tests.clone().unwrap_or_else(|| root.join(".oss").join("clang-tests"));
    let base = tests.join("clang").join("test");
    if !base.is_dir() {
        return Err(format!(
            "no clang tests at {} (run scripts/fetch-clang-tests.sh or pass --tests <llvm-project>)",
            base.display()
        ));
    }
    let paths: Vec<String> = if opts.paths.is_empty() {
        DEFAULT_DIRS.iter().map(|&d| d.to_owned()).collect()
    } else {
        opts.paths.clone()
    };
    let mut rels = discover(&base, &paths)?;
    if let Some(f) = &opts.filter {
        rels.retain(|r| glob_match(f, r));
    }
    let files: Vec<(String, String)> = rels
        .into_iter()
        .filter_map(|r| {
            let bytes = std::fs::read(base.join(&r)).ok()?;
            Some((r, String::from_utf8_lossy(&bytes).into_owned()))
        })
        .collect();
    let (jobs, mut skips, verify_files) = plan_all(&base, &files);
    if opts.list {
        for j in &jobs {
            println!("{} [{}] {}", j.file, j.cfg.label(), j.cfg.args.join(" "));
        }
        for (reason, s) in skips.ranked() {
            println!("skip {:>5} configs / {:>4} files  {reason}", s.configs, s.files.len());
        }
        println!("{} files, {verify_files} with -verify RUN lines, {} cases", files.len(), jobs.len());
        return Ok(());
    }

    let target = target_dir(&root)?;
    let kiln = match &opts.compiler {
        Some(c) => c.clone(),
        None => build_kiln_with(&root, &target, "clang-neg", opts.profile)?,
    };
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.subsec_nanos());
    let scratch =
        target.join("xtask-clang-neg").join(format!("run-{}-{nanos}", std::process::id()));
    std::fs::create_dir_all(&scratch)
        .map_err(|e| format!("cannot create {}: {e}", scratch.display()))?;
    let cfg = Config {
        kiln: kiln.clone(),
        base: base.clone(),
        scratch: scratch.clone(),
        timeout: opts.timeout,
        mem: MemLimit::from_mib(opts.mem_limit_mib),
        probe: RssProbe::detect(&scratch),
    };
    println!(
        "xtask clang-neg: {} cases from {} files under {} with {} (-j {}{})",
        jobs.len(),
        files.len(),
        base.display(),
        kiln.display(),
        opts.jobs,
        crate::jobs::global().describe().map_or_else(String::new, |d| format!(", jobserver {d}")),
    );
    let results = run_all(&cfg, &jobs, opts.jobs, opts.verbose);
    let _ = std::fs::remove_dir_all(&scratch);
    for r in &results {
        if let Status::FlagRejected(reason) = &r.status {
            skips.add(reason, &r.file, 1);
        }
    }
    let info = run_info(opts, &cfg, &root, &tests, &paths, files.len(), verify_files, start.elapsed());
    let summary = report::Summary::of(&results);
    println!("\n{}", summary.line());
    let json_path = opts
        .json
        .clone()
        .unwrap_or_else(|| target.join("xtask-clang-neg").join("report.json"));
    if let Some(d) = json_path.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    std::fs::write(&json_path, report::json(&info, &results, &skips).to_pretty())
        .map_err(|e| format!("cannot write {}: {e}", json_path.display()))?;
    println!("xtask clang-neg: wrote {}", json_path.display());
    if let Some(path) = &opts.report {
        let region = report::render(&info, &results, &skips);
        let existing = std::fs::read_to_string(path).unwrap_or_default();
        std::fs::write(path, report::splice(&existing, &region))
            .map_err(|e| format!("cannot write {}: {e}", path.display()))?;
        println!("xtask clang-neg: wrote {}", path.display());
    }
    if let Some(r) = results.iter().find(|r| matches!(r.status, Status::ToolError(_))) {
        return Err(r.console_line());
    }
    Ok(())
}

fn run_all(cfg: &Config, jobs: &[Job<'_>], n: usize, verbose: bool) -> Vec<CaseResult> {
    let next = AtomicUsize::new(0);
    let done = AtomicUsize::new(0);
    let slots: Mutex<Vec<Option<CaseResult>>> = Mutex::new(vec![None; jobs.len()]);
    std::thread::scope(|s| {
        for _ in 0..n.max(1) {
            s.spawn(|| {
                loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    let Some(j) = jobs.get(i) else { break };
                    let r = run_case(cfg, j, i);
                    let finished = done.fetch_add(1, Ordering::Relaxed) + 1;
                    let notable = !matches!(r.status, Status::Ran(_)) || r.rejects_valid();
                    if verbose || notable {
                        println!("[{finished}/{}] {}", jobs.len(), r.console_line());
                    } else if finished % 500 == 0 {
                        println!("[{finished}/{}]", jobs.len());
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
        .map(|r| r.expect("every case produced a result"))
        .collect()
}

#[allow(clippy::too_many_arguments)] // one call site; the fields of the page header
fn run_info(
    opts: &Options,
    cfg: &Config,
    root: &Path,
    tests: &Path,
    paths: &[String],
    files: usize,
    verify_files: usize,
    wall: Duration,
) -> report::RunInfo {
    let git = |dir: &Path, args: &[&str]| {
        Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(args)
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map_or_else(
                || "unknown".to_owned(),
                |o| String::from_utf8_lossy(&o.stdout).trim().to_owned(),
            )
    };
    let mut command = format!("cargo xtask clang-neg -j {}", opts.jobs);
    match &opts.compiler {
        Some(_) => command.push_str(" --compiler <kiln++>"),
        None => {
            let _ = write!(command, " --profile {}", opts.profile.label());
        }
    }
    if let Some(f) = &opts.filter {
        let _ = write!(command, " --filter '{f}'");
    }
    if opts.paths.is_empty() {
        let _ = write!(command, " {}", DEFAULT_DIRS.join(" "));
    } else {
        let _ = write!(command, " {}", paths.join(" "));
    }
    report::RunInfo {
        kiln_version: crate::util::identify(&cfg.kiln)
            .map_or_else(|_| "unknown".to_owned(), |id| id.version),
        kiln_commit: git(root, &["rev-parse", "--short", "HEAD"]),
        clang_tag: CLANG_TAG.to_owned(),
        clang_commit: git(tests, &["rev-parse", "HEAD"]),
        command,
        limits: format!(
            "{} per case (plus kiln's stack reservation); timeout {} s; -j {}",
            cfg.mem.map_or_else(|| "no memory cap".to_owned(), |m| format!("{} MiB address space", m.mib)),
            cfg.timeout.as_secs_f64(),
            opts.jobs
        ),
        rss_source: cfg.probe.source_id().to_owned(),
        files,
        verify_files,
        wall,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Option<Options>, String> {
        parse_args(args.iter().map(|s| (*s).to_owned()))
    }

    #[test]
    fn options() {
        let o = parse(&["-j", "3", "--filter", "CXX/*", "--json=r.json", "CXX/", "SemaCXX/a.cpp"])
            .unwrap()
            .unwrap();
        assert_eq!(o.jobs, 3);
        assert_eq!(o.filter.as_deref(), Some("CXX/*"));
        assert_eq!(o.json, Some(PathBuf::from("r.json")));
        assert_eq!(o.paths, ["CXX", "SemaCXX/a.cpp"]);
        assert_eq!(parse(&["--help"]), Ok(None));
        assert!(parse(&["-j", "0"]).is_err());
        assert!(parse(&["--list=1"]).is_err());
        assert!(parse(&["--bogus"]).is_err());
        let d = Options::default();
        assert_eq!((d.jobs, d.mem_limit_mib, d.timeout), (4, 2048, Duration::from_secs(60)));
    }

    /// spec/clang-neg.md "Report": the per-directory rows.
    #[test]
    fn directory_keys() {
        assert_eq!(dir_key("CXX/dcl.decl/dcl.init/p1.cpp"), "CXX/dcl.decl");
        assert_eq!(dir_key("CXX/drs/cwg1xx.cpp"), "CXX/drs");
        assert_eq!(dir_key("CXX/top.cpp"), "CXX");
        assert_eq!(dir_key("SemaCXX/attr-x.cpp"), "SemaCXX");
        assert_eq!(dir_key("SemaCXX/sub/x.cpp"), "SemaCXX");
    }

    /// spec/clang-neg.md "RUN lines": REQUIRES skips every case, duplicate
    /// configurations collapse.
    #[test]
    fn files_are_planned() {
        let text = "// RUN: %clang_cc1 -fsyntax-only -verify %s\n\
                    // RUN: %clang_cc1 -fsyntax-only -verify -emit-llvm-only %s\n\
                    // RUN: %clang_cc1 -fsyntax-only -verify -fms-extensions %s\n\
                    // RUN: %clang_cc1 -emit-llvm %s -o - | FileCheck %s\n";
        let fp = plan_file(text, "/d");
        assert!(fp.verify);
        assert_eq!(fp.cases.len(), 1);
        assert_eq!(fp.skips.len(), 2);
        let req = format!("// REQUIRES: powerpc-registered-target\n{text}");
        let fp = plan_file(&req, "/d");
        assert!(fp.cases.is_empty());
        assert!(fp.skips.iter().any(|(r, n)| r.starts_with("REQUIRES") && *n == 1));
        assert!(!plan_file("// RUN: %clang_cc1 -emit-llvm %s\n", "/d").verify);
    }

    /// spec/clang-neg.md "Classification": exit status and driver errors.
    #[cfg(unix)]
    #[test]
    fn runs_are_judged() {
        use std::os::unix::process::ExitStatusExt;
        let code = |c: i32| Some(std::process::ExitStatus::from_raw(c << 8));
        assert_eq!(judge(None, ""), Status::Timeout);
        assert_eq!(
            judge(code(1), "kiln++: error: unknown argument: '-fxyz'\n"),
            Status::FlagRejected("kiln: unknown argument: '-fxyz'".into())
        );
        assert!(matches!(
            judge(code(101), "thread 'main' panicked at crates/a.rs:1:2:\nboom\n"),
            Status::Crash(_)
        ));
        assert!(matches!(judge(Some(std::process::ExitStatus::from_raw(11)), ""), Status::Crash(_)));
        let ok = judge(code(1), "error: 'expected-error' diagnostics expected but not seen: \n  File a.cpp Line 3: KILN-NEG x\n");
        assert!(matches!(ok, Status::Ran(ref c) if c.accepts_invalid()));
        assert_eq!(judge(code(0), ""), Status::Ran(Classified::default()));
    }

    #[test]
    fn case_arguments() {
        let cfg = CaseCfg {
            run: 1,
            std: "c++11".into(),
            prefixes: vec!["expected".into(), "cxx11".into()],
            args: vec!["-fno-exceptions".into(), "-include".into(), "%s".into()],
        };
        assert_eq!(
            kiln_args(&cfg, Path::new("/t/CXX"), Path::new("/s/1/p1.cpp")),
            [
                "-fsyntax-only",
                "-ferror-limit=0",
                "-std=c++11",
                "-verify=expected,cxx11",
                "-iquote",
                "/t/CXX",
                "-fno-exceptions",
                "-include",
                "/s/1/p1.cpp"
            ]
        );
    }
}
