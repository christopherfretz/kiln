//! `cargo xtask gcc-torture`: GCC's C torture suites (`gcc.c-torture/
//! {execute,execute/ieee,execute/builtins,compile}`) compiled by kiln at
//! each option set, linked and run, beside GCC 16.1 and clang on the same
//! cases (spec/harness.md "GCC testsuites"). Informational: the exit
//! status is non-zero only when the tool itself fails.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use super::dg::{self, Action, Plan, RunPlan, SuiteDefaults};
use super::outcome::{self, Class};
use super::target::Target;
use super::{Case, GCC_TAG, Ran, Role, Tool, xfile};
use crate::libstdcxx::glob_match;
use crate::util::process::MemLimit;
use crate::util::{Profile, build_kiln_with, repo_root, target_dir};

const USAGE: &str = "\
usage: cargo xtask gcc-torture [options]

Runs GCC's C torture tests (gcc.c-torture, GCC 16.1) through kiln at each
option set: compile, link and run each test, then the same case through GCC
16.1 and clang. Each case is classed pass, compile-reject, crash, wrong
(the program failed), timeout or accept-invalid per compiler. Writes a
per-case TSV and a Markdown summary. Honours dg-do, dg-options,
dg-additional-options, dg-skip-if, dg-require-effective-target,
dg-xfail-if and the .x files of the c-torture-execute suites.
Informational: the exit status is non-zero only when the tool fails.
Contract: spec/harness.md \"GCC testsuites\".

  --tests <dir>          a GCC checkout holding gcc/testsuite (default:
                         .oss/gcc-tests, fetched by scripts/fetch-gcc-tests.sh
                         on first use)
  --suite <list>         comma-separated: execute, ieee, builtins, compile
                         (default: execute,ieee,builtins)
  --opt <list>           comma-separated option sets (default: -O0,-O2)
  --std <std>            -std=<std> before every test's flags (default
                         gnu23, GCC 16's default; `none` passes none)
  --profile <p>          the cargo profile to build kiln with: debug, harness
                         (default) or release
  --compiler <path>      kiln's C driver (default: `kiln` beside the kiln++
                         that cargo build --profile <p> -p kiln_driver makes)
  --gcc <path>           GCC's C driver (default: $KILN_GCC with g++ -> gcc)
  --clang <path>         clang's C driver (default: clang-19)
  --oracles <list>       gcc,clang (default), gcc, clang or none
  --filter <glob>        only tests whose id (execute/foo.c) matches
  --list                 list the planned cases and skips, run nothing
  -j, --jobs <n>         cases in parallel (default 4; an upper bound under a
                         MAKEFLAGS jobserver, see spec/harness.md)
  --timeout <secs>       per compile or link step (default 120)
  --run-timeout <secs>   per test run, times dg-timeout-factor (default 10)
  --mem-limit <MiB>      address-space cap per step, plus kiln's stack
                         reservation (default 2048; 0: none)
  --tsv <file>           the per-case TSV (default: <target>/xtask-gcc-torture/results.tsv)
  --summary <file.md>    the summary (default: <target>/xtask-gcc-torture/summary.md)
  --report <file.md>     also splice the summary into this page
  -v, --verbose          print every case's result
";

/// A torture suite and how GCC's `.exp` driver runs it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Suite {
    /// `execute/*.c`: gcc-dg-runtest, `run` by default, `-w`.
    Execute,
    /// `execute/ieee/*.c`: c-torture-execute with `-fno-inline` and `.x`.
    Ieee,
    /// `execute/builtins/*.c` with `<test>-lib.c` and `lib/main.c`.
    Builtins,
    /// `compile/*.c`: gcc-dg-runtest, `assemble` by default, `-w`.
    Compile,
}

impl Suite {
    pub const ALL: [Self; 4] = [Self::Execute, Self::Ieee, Self::Builtins, Self::Compile];

    pub fn name(self) -> &'static str {
        match self {
            Self::Execute => "execute",
            Self::Ieee => "ieee",
            Self::Builtins => "builtins",
            Self::Compile => "compile",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|x| x.name() == s)
    }

    /// The directory under `gcc.c-torture`.
    pub fn rel_dir(self) -> &'static str {
        match self {
            Self::Execute => "execute",
            Self::Ieee => "execute/ieee",
            Self::Builtins => "execute/builtins",
            Self::Compile => "compile",
        }
    }

    /// The `additional_flags` a c-torture-execute suite starts from (its
    /// `.exp` for a hosted LP64 GNU/Linux target); `None` for the dg ones.
    pub fn torture_flags(self) -> Option<&'static str> {
        match self {
            Self::Ieee => Some("-fno-inline"),
            Self::Builtins => Some(
                "-fno-tree-dse -fno-tree-loop-distribute-patterns -fno-tracer -fno-ipa-ra \
                 -fno-inline-functions",
            ),
            Self::Execute | Self::Compile => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Options {
    pub tests: Option<PathBuf>,
    pub suites: Vec<Suite>,
    pub opts: Vec<String>,
    pub std: Option<String>,
    pub profile: Profile,
    pub compiler: Option<PathBuf>,
    pub gcc: Option<PathBuf>,
    pub clang: PathBuf,
    pub oracles: Vec<Role>,
    pub filter: Option<String>,
    pub list: bool,
    pub jobs: usize,
    pub timeout: Duration,
    pub run_timeout: Duration,
    pub mem_limit_mib: u64,
    pub tsv: Option<PathBuf>,
    pub summary: Option<PathBuf>,
    pub report: Option<PathBuf>,
    pub verbose: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            tests: None,
            suites: vec![Suite::Execute, Suite::Ieee, Suite::Builtins],
            opts: vec!["-O0".into(), "-O2".into()],
            std: Some("gnu23".into()),
            profile: Profile::Harness,
            compiler: None,
            gcc: None,
            clang: PathBuf::from("clang-19"),
            oracles: vec![Role::Gcc, Role::Clang],
            filter: None,
            list: false,
            jobs: 4,
            timeout: Duration::from_secs(120),
            run_timeout: Duration::from_secs(10),
            mem_limit_mib: 2048,
            tsv: None,
            summary: None,
            report: None,
            verbose: false,
        }
    }
}

fn secs(flag: &str, v: &str) -> Result<Duration, String> {
    v.parse::<f64>()
        .ok()
        .filter(|s| s.is_finite() && *s > 0.0)
        .map(Duration::from_secs_f64)
        .ok_or_else(|| format!("bad {flag} `{v}`"))
}

fn comma_list(v: &str) -> Vec<String> {
    v.split(',').map(str::trim).filter(|s| !s.is_empty()).map(str::to_owned).collect()
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
            "--suite" => {
                o.suites = comma_list(&value("--suite")?)
                    .iter()
                    .map(|s| Suite::parse(s).ok_or_else(|| format!("unknown suite `{s}`")))
                    .collect::<Result<_, _>>()?;
                if o.suites.is_empty() {
                    return Err("`--suite` names no suite".into());
                }
            }
            "--opt" => {
                o.opts = comma_list(&value("--opt")?);
                if o.opts.is_empty() {
                    return Err("`--opt` names no option set".into());
                }
            }
            "--std" => {
                let v = value("--std")?;
                o.std = (v != "none").then_some(v);
            }
            "--profile" => o.profile = Profile::parse(&value("--profile")?)?,
            "--compiler" => o.compiler = Some(PathBuf::from(value("--compiler")?)),
            "--gcc" => o.gcc = Some(PathBuf::from(value("--gcc")?)),
            "--clang" => o.clang = PathBuf::from(value("--clang")?),
            "--oracles" => {
                let v = value("--oracles")?;
                o.oracles = Vec::new();
                for r in comma_list(&v) {
                    match r.as_str() {
                        "gcc" => o.oracles.push(Role::Gcc),
                        "clang" => o.oracles.push(Role::Clang),
                        "none" => {}
                        _ => return Err(format!("unknown oracle `{r}` (gcc, clang or none)")),
                    }
                }
                o.oracles.sort();
                o.oracles.dedup();
            }
            "--filter" => o.filter = Some(value("--filter")?),
            "--tsv" => o.tsv = Some(PathBuf::from(value("--tsv")?)),
            "--summary" => o.summary = Some(PathBuf::from(value("--summary")?)),
            "--report" => o.report = Some(PathBuf::from(value("--report")?)),
            "-j" | "--jobs" => {
                let v = value("--jobs")?;
                o.jobs =
                    v.parse().ok().filter(|&n| n > 0).ok_or_else(|| format!("bad --jobs `{v}`"))?;
            }
            "--timeout" => o.timeout = secs("--timeout", &value("--timeout")?)?,
            "--run-timeout" => o.run_timeout = secs("--run-timeout", &value("--run-timeout")?)?,
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
            eprintln!("xtask gcc-torture: {e}\n\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    match execute(&opts) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("xtask gcc-torture: {e}");
            ExitCode::from(2)
        }
    }
}

/// One test of a suite: its id (`execute/ieee/fp-cmp-1.c`) and sources.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Test {
    pub suite: Suite,
    pub id: String,
    pub sources: Vec<PathBuf>,
}

/// The suite's tests under `torture` (`gcc/testsuite/gcc.c-torture`),
/// sorted: the top-level `*.c` of its directory (builtins: not `*-lib.c`,
/// with `<test>-lib.c` when present and `lib/main.c`).
pub fn discover(torture: &Path, suite: Suite) -> Result<Vec<Test>, String> {
    let dir = torture.join(suite.rel_dir());
    let rd = std::fs::read_dir(&dir).map_err(|e| format!("cannot read {}: {e}", dir.display()))?;
    let mut names: Vec<String> = rd
        .filter_map(Result::ok)
        .filter(|e| e.path().is_file())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| Path::new(n).extension().is_some_and(|e| e == "c"))
        .collect();
    names.sort();
    let mut out = Vec::new();
    for n in names {
        let src = dir.join(&n);
        let mut sources = vec![src];
        if suite == Suite::Builtins {
            if n.ends_with("-lib.c") {
                continue;
            }
            let lib = dir.join(format!("{}-lib.c", n.trim_end_matches(".c")));
            if lib.is_file() {
                sources.push(lib);
            }
            sources.push(dir.join("lib").join("main.c"));
        }
        out.push(Test { suite, id: format!("{}/{n}", suite.rel_dir()), sources });
    }
    Ok(out)
}

/// A test's plan per option set, in `opts` order.
pub fn plan_test(
    test: &Test,
    body: &str,
    x: Option<&str>,
    opts: &[String],
    t: &Target,
) -> Vec<Plan> {
    match test.suite.torture_flags() {
        None => {
            let defaults = SuiteDefaults {
                action: if test.suite == Suite::Compile { Action::Assemble } else { Action::Run },
                extra_flags: vec!["-w".into()],
            };
            let dirs = dg::directives(body);
            opts.iter().map(|o| dg::resolve(&dirs, &defaults, t, &dg::split_flags(o))).collect()
        }
        Some(base) => {
            let xr = match x.map(|x| xfile::eval(x, t, base)) {
                None => xfile::XResult { additional_flags: base.to_owned(), ..Default::default() },
                Some(Ok(r)) => r,
                Some(Err(e)) => {
                    return opts.iter().map(|_| Plan::Skip(format!(".x: {e}"))).collect();
                }
            };
            if xr.skip {
                return opts.iter().map(|_| Plan::Skip(".x returns 1".into())).collect();
            }
            let mut flags = dg::split_flags(&xr.additional_flags);
            flags.push("-w".into());
            opts.iter()
                .map(|o| {
                    if let Some(body) = &xr.before_compile {
                        match xfile::option_allowed(body, t, o) {
                            Err(e) => return Plan::Skip(format!(".x: {e}")),
                            Ok(false) => {
                                return Plan::Skip(".x torture_eval_before_compile".into());
                            }
                            Ok(true) => {}
                        }
                    }
                    Plan::Run(RunPlan {
                        action: Action::Run,
                        flags: flags.clone(),
                        timeout_factor: 1.0,
                        xfail: xr.xfail,
                        negative: false,
                    })
                })
                .collect()
        }
    }
}

/// One scheduled case: a test at one option set.
#[derive(Clone, Debug)]
pub struct Job {
    pub test: usize,
    pub opt: String,
    pub plan: RunPlan,
    /// `-std=`, the option set, then the test's flags.
    pub flags: Vec<String>,
}

/// A case's row: the run's results, or a skip (`opt` `*` when the test
/// skips at every option set for the same reason).
#[derive(Clone, Debug)]
pub struct Row {
    pub test: String,
    pub suite: Suite,
    pub opt: String,
    pub skip: Option<String>,
    pub xfail: bool,
    pub results: BTreeMap<Role, Ran>,
}

impl Row {
    pub fn class(&self, r: Role) -> Option<Class> {
        self.results.get(&r).map(|x| x.verdict.class)
    }
}

/// The jobs and the skip rows for the selected tests.
pub fn plan_all(
    tests: &[Test],
    bodies: &[(String, Option<String>)],
    opts: &[String],
    std: Option<&str>,
    t: &Target,
) -> (Vec<Job>, Vec<Row>) {
    let mut jobs = Vec::new();
    let mut skips = Vec::new();
    for (i, (test, (body, x))) in tests.iter().zip(bodies).enumerate() {
        let plans = plan_test(test, body, x.as_deref(), opts, t);
        let skip_row = |opt: &str, why: &str| Row {
            test: test.id.clone(),
            suite: test.suite,
            opt: opt.to_owned(),
            skip: Some(why.to_owned()),
            xfail: false,
            results: BTreeMap::new(),
        };
        let first_skip = match plans.first() {
            Some(Plan::Skip(w)) => Some(w.clone()),
            _ => None,
        };
        if let Some(w) = first_skip
            && plans.iter().all(|p| matches!(p, Plan::Skip(x) if *x == w))
        {
            skips.push(skip_row("*", &w));
            continue;
        }
        for (opt, p) in opts.iter().zip(plans) {
            match p {
                Plan::Skip(w) => skips.push(skip_row(opt, &w)),
                Plan::Run(plan) => {
                    let mut flags: Vec<String> =
                        std.map(|s| format!("-std={s}")).into_iter().collect();
                    flags.extend(dg::split_flags(opt));
                    flags.extend(plan.flags.iter().cloned());
                    jobs.push(Job { test: i, opt: opt.clone(), plan, flags });
                }
            }
        }
    }
    (jobs, skips)
}

struct Config {
    tools: Vec<Tool>,
    scratch: PathBuf,
    timeout: Duration,
    run_timeout: Duration,
    mem: Option<MemLimit>,
}

const LINK_FLAGS: &[&str] = &["-lm"];

fn run_job(cfg: &Config, tests: &[Test], job: &Job, idx: usize) -> Row {
    let test = &tests[job.test];
    let link: Vec<String> = LINK_FLAGS.iter().map(|&s| s.to_owned()).collect();
    let case = Case {
        sources: &test.sources,
        action: job.plan.action,
        flags: &job.flags,
        link_flags: &link,
        negative: job.plan.negative,
        compile_timeout: cfg.timeout.mul_f64(job.plan.timeout_factor.max(1.0)),
        run_timeout: cfg.run_timeout.mul_f64(job.plan.timeout_factor.max(1.0)),
        mem: cfg.mem,
    };
    let dir = cfg.scratch.join(idx.to_string());
    let mut results = BTreeMap::new();
    for tool in &cfg.tools {
        results.insert(tool.role, tool.run_case(&case, &dir.join(tool.role.name())));
    }
    let _ = std::fs::remove_dir_all(&dir);
    Row {
        test: test.id.clone(),
        suite: test.suite,
        opt: job.opt.clone(),
        skip: None,
        xfail: job.plan.xfail,
        results,
    }
}

fn console_line(row: &Row) -> String {
    let mut s = format!("{} [{}]", row.test, row.opt);
    for (r, ran) in &row.results {
        let _ = write!(s, " {}={}", r.name(), ran.verdict.class.label());
        if ran.verdict.class.is_failure() && *r == Role::Kiln {
            let _ = write!(s, " ({})", ran.verdict.detail);
        }
    }
    s
}

fn run_all(cfg: &Config, tests: &[Test], jobs: &[Job], n: usize, verbose: bool) -> Vec<Row> {
    let next = AtomicUsize::new(0);
    let done = AtomicUsize::new(0);
    let slots: Mutex<Vec<Option<Row>>> = Mutex::new(vec![None; jobs.len()]);
    std::thread::scope(|s| {
        for _ in 0..n.max(1) {
            s.spawn(|| {
                loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    let Some(j) = jobs.get(i) else { break };
                    let r = run_job(cfg, tests, j, i);
                    let finished = done.fetch_add(1, Ordering::Relaxed) + 1;
                    if verbose {
                        println!("[{finished}/{}] {}", jobs.len(), console_line(&r));
                    } else if finished.is_multiple_of(250) {
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

fn resolve_tools(opts: &Options, root: &Path, scratch: &Path) -> Result<Vec<Tool>, String> {
    let llvm = crate::oracle::Oracle::from_env().unwrap_or_default().llvm_dir();
    let kiln_env = vec![("KILN_LLVM_DIR".to_owned(), llvm.display().to_string())];
    let kiln = if let Some(c) = &opts.compiler {
        c.clone()
    } else {
        let target = target_dir(root)?;
        let kpp = build_kiln_with(root, &target, "gcc-torture", opts.profile)?;
        kpp.with_file_name(format!("kiln{}", std::env::consts::EXE_SUFFIX))
    };
    let mut tools = vec![Tool::new(Role::Kiln, kiln, kiln_env, scratch)?];
    if opts.oracles.contains(&Role::Gcc) {
        let (gcc, explicit) = if let Some(g) = &opts.gcc {
            (g.clone(), true)
        } else {
            let env = std::env::var(crate::oracle::GCC_ENV).ok();
            let (gxx, explicit) = crate::oracle::gcc_from(env.as_deref());
            (gcc_beside(&gxx), explicit)
        };
        match Tool::new(Role::Gcc, gcc, Vec::new(), scratch) {
            Ok(t) => tools.push(t),
            Err(e) if explicit => return Err(e),
            Err(e) => eprintln!("xtask gcc-torture: no GCC oracle ({e}); set KILN_GCC"),
        }
    }
    if opts.oracles.contains(&Role::Clang) {
        let clang = Tool::new(Role::Clang, opts.clang.clone(), Vec::new(), scratch)?;
        // kiln emulates the clang oracle's release, as `xtask test` does
        // (spec/harness.md): clang 22 makes some C warnings errors.
        if let Some(m) = clang.clang_major().filter(|m| crate::oracle::SUPPORTED.contains(m)) {
            tools[0].lead_with(format!("-fclang-compat={m}"));
        }
        tools.push(clang);
    }
    Ok(tools)
}

/// The C driver beside a `g++` (`.../g++-16` -> `.../gcc-16`); anything
/// else is taken as the C driver itself.
pub fn gcc_beside(gxx: &Path) -> PathBuf {
    match gxx.file_name().and_then(|n| n.to_str()) {
        Some(n) if n.contains("g++") => gxx.with_file_name(n.replacen("g++", "gcc", 1)),
        _ => gxx.to_path_buf(),
    }
}

#[allow(clippy::too_many_lines, reason = "one linear plan / run / report sequence")]
fn execute(opts: &Options) -> Result<(), String> {
    let start = Instant::now();
    let root = repo_root();
    let testsuite = super::locate(&root, opts.tests.as_deref())?;
    let torture = testsuite.join("gcc.c-torture");
    let target = Target::host();
    let mut tests = Vec::new();
    for &s in &opts.suites {
        tests.extend(discover(&torture, s)?);
    }
    if let Some(f) = &opts.filter {
        tests.retain(|t| glob_match(f, &t.id));
    }
    let bodies: Vec<(String, Option<String>)> = tests
        .iter()
        .map(|t| {
            let read =
                |p: &Path| std::fs::read(p).ok().map(|b| String::from_utf8_lossy(&b).into_owned());
            let x = t.suite.torture_flags().and_then(|_| read(&t.sources[0].with_extension("x")));
            (read(&t.sources[0]).unwrap_or_default(), x)
        })
        .collect();
    let (jobs, skips) = plan_all(&tests, &bodies, &opts.opts, opts.std.as_deref(), &target);
    if opts.list {
        for j in &jobs {
            println!(
                "{} [{}] {} {}",
                tests[j.test].id,
                j.opt,
                j.plan.action.name(),
                j.flags.join(" ")
            );
        }
        for s in &skips {
            println!("{} [{}] skip: {}", s.test, s.opt, s.skip.as_deref().unwrap_or(""));
        }
        for (why, n) in ranked_skips(&skips) {
            println!("skip {n:>5}  {why}");
        }
        println!("{} tests, {} cases, {} skip rows", tests.len(), jobs.len(), skips.len());
        return Ok(());
    }

    let tdir = target_dir(&root)?;
    let out_dir = tdir.join("xtask-gcc-torture");
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.subsec_nanos());
    let scratch = out_dir.join(format!("run-{}-{nanos}", std::process::id()));
    std::fs::create_dir_all(&scratch)
        .map_err(|e| format!("cannot create {}: {e}", scratch.display()))?;
    let tools = resolve_tools(opts, &root, &scratch)?;
    let cfg = Config {
        tools,
        scratch: scratch.clone(),
        timeout: opts.timeout,
        run_timeout: opts.run_timeout,
        mem: MemLimit::from_mib(opts.mem_limit_mib),
    };
    println!(
        "xtask gcc-torture: {} cases from {} tests ({} skip rows) under {}, compilers {} (-j {}{})",
        jobs.len(),
        tests.len(),
        skips.len(),
        torture.display(),
        cfg.tools.iter().map(|t| t.path.display().to_string()).collect::<Vec<_>>().join(", "),
        opts.jobs,
        crate::jobs::global().describe().map_or_else(String::new, |d| format!(", jobserver {d}")),
    );
    let mut rows = run_all(&cfg, &tests, &jobs, opts.jobs, opts.verbose);
    let _ = std::fs::remove_dir_all(&scratch);
    rows.extend(skips);
    rows.sort_by(|a, b| (&a.test, &a.opt).cmp(&(&b.test, &b.opt)));

    let roles: Vec<Role> = cfg.tools.iter().map(|t| t.role).collect();
    let info = RunInfo {
        tests_commit: git_head(&testsuite),
        kiln_commit: git_short(&root),
        command: command_line(opts),
        compilers: cfg.tools.iter().map(|t| (t.role, t.version.clone())).collect(),
        dropped: cfg.tools.iter().map(|t| (t.role, t.rejected())).collect(),
        opts: opts.opts.clone(),
        std: opts.std.clone(),
        triplet: target.triplet.clone(),
        tests: tests.len(),
        limits: format!(
            "{}; compile/link timeout {} s, run timeout {} s; -j {}",
            cfg.mem.map_or_else(
                || "no memory cap".to_owned(),
                |m| format!("{} MiB address space per step (plus kiln's stack reservation)", m.mib)
            ),
            opts.timeout.as_secs_f64(),
            opts.run_timeout.as_secs_f64(),
            opts.jobs
        ),
        wall: start.elapsed(),
    };
    let tsv_path = opts.tsv.clone().unwrap_or_else(|| out_dir.join("results.tsv"));
    let md_path = opts.summary.clone().unwrap_or_else(|| out_dir.join("summary.md"));
    for p in [&tsv_path, &md_path] {
        if let Some(d) = p.parent() {
            let _ = std::fs::create_dir_all(d);
        }
    }
    std::fs::write(&tsv_path, tsv(&rows, &roles))
        .map_err(|e| format!("cannot write {}: {e}", tsv_path.display()))?;
    let region = render(&info, &rows, &roles);
    std::fs::write(&md_path, &region)
        .map_err(|e| format!("cannot write {}: {e}", md_path.display()))?;
    println!("\n{}", summary_line(&rows, &roles));
    println!("xtask gcc-torture: wrote {} and {}", tsv_path.display(), md_path.display());
    if let Some(path) = &opts.report {
        let existing = std::fs::read_to_string(path).unwrap_or_default();
        std::fs::write(path, splice(&existing, &region, opts.std.as_deref()))
            .map_err(|e| format!("cannot write {}: {e}", path.display()))?;
        println!("xtask gcc-torture: wrote {}", path.display());
    }
    Ok(())
}

fn git_out(dir: &Path, args: &[&str]) -> String {
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
}

fn git_head(dir: &Path) -> String {
    git_out(dir, &["rev-parse", "HEAD"])
}

fn git_short(dir: &Path) -> String {
    git_out(dir, &["rev-parse", "--short", "HEAD"])
}

fn command_line(o: &Options) -> String {
    let mut c = format!("cargo xtask gcc-torture -j {}", o.jobs);
    let suites: Vec<&str> = o.suites.iter().map(|s| s.name()).collect();
    let _ = write!(c, " --suite {} --opt {}", suites.join(","), o.opts.join(","));
    if let Some(s) = &o.std {
        if s != "gnu23" {
            let _ = write!(c, " --std {s}");
        }
    } else {
        c.push_str(" --std none");
    }
    match &o.compiler {
        Some(p) => {
            let _ = write!(c, " --compiler {}", p.display());
        }
        None => {
            let _ = write!(c, " --profile {}", o.profile.label());
        }
    }
    if let Some(f) = &o.filter {
        let _ = write!(c, " --filter '{f}'");
    }
    c
}

/// What the summary's header states.
#[derive(Clone, Debug, Default)]
pub struct RunInfo {
    pub tests_commit: String,
    pub kiln_commit: String,
    pub command: String,
    pub compilers: Vec<(Role, String)>,
    pub dropped: Vec<(Role, Vec<String>)>,
    pub opts: Vec<String>,
    pub std: Option<String>,
    pub triplet: String,
    pub tests: usize,
    pub limits: String,
    pub wall: Duration,
}

fn cell(s: &str) -> String {
    s.replace(['\t', '\n', '\r'], " ")
}

/// The per-case TSV: one row per (test, option set), skips included.
pub fn tsv(rows: &[Row], roles: &[Role]) -> String {
    let mut o = String::from("test\topt");
    for r in [Role::Kiln, Role::Gcc, Role::Clang] {
        let _ = write!(o, "\t{0}\t{0}_detail", r.name());
    }
    o.push_str("\txfail\tdropped\n");
    for row in rows {
        let _ = write!(o, "{}\t{}", cell(&row.test), cell(&row.opt));
        for r in [Role::Kiln, Role::Gcc, Role::Clang] {
            match (&row.skip, row.results.get(&r)) {
                (Some(why), _) if roles.contains(&r) => {
                    let _ = write!(o, "\tskip\t{}", cell(why));
                }
                (_, Some(ran)) => {
                    let _ =
                        write!(o, "\t{}\t{}", ran.verdict.class.label(), cell(&ran.verdict.detail));
                }
                _ => o.push_str("\t-\t"),
            }
        }
        let dropped: Vec<String> = row
            .results
            .iter()
            .filter(|(_, ran)| !ran.dropped.is_empty())
            .map(|(r, ran)| format!("{}:{}", r.name(), ran.dropped.join(",")))
            .collect();
        let _ = writeln!(
            o,
            "\t{}\t{}",
            if row.xfail { "xfail" } else { "" },
            cell(&dropped.join("; "))
        );
    }
    o
}

fn ranked_skips(rows: &[Row]) -> Vec<(String, usize)> {
    let mut m: BTreeMap<String, usize> = BTreeMap::new();
    for r in rows {
        if let Some(w) = &r.skip {
            *m.entry(w.clone()).or_default() += 1;
        }
    }
    let mut v: Vec<_> = m.into_iter().collect();
    v.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    v
}

/// The cases each failure cluster of `role` holds, largest first, with up
/// to three example cases.
pub fn clusters(rows: &[Row], role: Role) -> Vec<(String, Vec<String>)> {
    let mut m: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for row in rows {
        if let Some(ran) = row.results.get(&role)
            && ran.verdict.class.is_failure()
        {
            m.entry(outcome::cluster_key(&ran.verdict))
                .or_default()
                .push(format!("{} {}", row.test, row.opt));
        }
    }
    let mut v: Vec<_> = m.into_iter().collect();
    v.sort_by(|a, b| b.1.len().cmp(&a.1.len()).then_with(|| a.0.cmp(&b.0)));
    v
}

fn ran_rows(rows: &[Row]) -> impl Iterator<Item = &Row> {
    rows.iter().filter(|r| r.skip.is_none())
}

/// `kiln: 1200/1500 pass, gcc: ..., clang: ...; N skip rows`.
pub fn summary_line(rows: &[Row], roles: &[Role]) -> String {
    let run = ran_rows(rows).count();
    let parts: Vec<String> = roles
        .iter()
        .map(|&r| {
            let pass = ran_rows(rows).filter(|x| x.class(r) == Some(Class::Pass)).count();
            format!("{}: {pass}/{run} pass", r.name())
        })
        .collect();
    format!("{}; {} skip rows", parts.join(", "), rows.len() - run)
}

pub const BEGIN: &str = "<!-- gcc-torture:begin (generated by cargo xtask gcc-torture) -->";
pub const END: &str = "<!-- gcc-torture:end -->";

/// The region markers for a run at `std` (WP-1282): the default gnu23 run
/// keeps [`BEGIN`]/[`END`]; any other `--std` gets its own region, so one
/// status page holds a gnu23 and a gnu17 run side by side.
pub fn markers(std: Option<&str>) -> (String, String) {
    match std {
        Some("gnu23") => (BEGIN.to_owned(), END.to_owned()),
        s => {
            let s = s.unwrap_or("none");
            (
                format!(
                    "<!-- gcc-torture:begin std={s} (generated by cargo xtask gcc-torture) -->"
                ),
                format!("<!-- gcc-torture:end std={s} -->"),
            )
        }
    }
}

fn md(s: &str) -> String {
    s.replace('|', "\\|")
}

/// The summary: the region `--report` splices into a status page.
#[allow(clippy::too_many_lines, reason = "one straight-line document")]
pub fn render(info: &RunInfo, rows: &[Row], roles: &[Role]) -> String {
    let mut o = String::new();
    let (begin, end) = markers(info.std.as_deref());
    let _ = writeln!(o, "{begin}\n");
    let _ = writeln!(
        o,
        "GCC C torture suites (`gcc.c-torture`, `{GCC_TAG}`), host target `{}`.\n",
        info.triplet
    );
    let _ = writeln!(o, "- tests: GCC `{}`; kiln `{}`", info.tests_commit, info.kiln_commit);
    let _ = writeln!(o, "- command: `{}`", info.command);
    for (r, v) in &info.compilers {
        let _ = writeln!(o, "- {}: {}", r.name(), md(v));
    }
    let _ = writeln!(
        o,
        "- option sets: {}; {}",
        info.opts.join(", "),
        info.std.as_ref().map_or_else(|| "no -std".to_owned(), |s| format!("-std={s} first"))
    );
    let _ = writeln!(o, "- limits: {}", info.limits);
    let run = ran_rows(rows).count();
    let _ = writeln!(
        o,
        "- {} tests, {run} cases run, {} skip rows; wall {:.0} s\n",
        info.tests,
        rows.len() - run,
        info.wall.as_secs_f64()
    );

    let _ = writeln!(o, "### Results by class (cases)\n");
    let head: Vec<&str> = roles.iter().map(|r| r.name()).collect();
    let _ = writeln!(o, "| class | {} |", head.join(" | "));
    let _ = writeln!(o, "|---|{}", "---:|".repeat(roles.len()));
    for c in Class::ALL {
        let counts: Vec<usize> = roles
            .iter()
            .map(|&r| ran_rows(rows).filter(|x| x.class(r) == Some(c)).count())
            .collect();
        if counts.iter().all(|&n| n == 0) {
            continue;
        }
        let cells: Vec<String> = counts.iter().map(ToString::to_string).collect();
        let _ = writeln!(o, "| {} | {} |", c.label(), cells.join(" | "));
    }
    let _ = writeln!(o, "| total | {} |\n", vec![run.to_string(); roles.len()].join(" | "));

    let _ = writeln!(o, "### Pass by suite and option set\n");
    let _ = writeln!(o, "| suite | opt | cases | {} |", head.join(" | "));
    let _ = writeln!(o, "|---|---|---:|{}", "---:|".repeat(roles.len()));
    let mut keys: Vec<(Suite, String)> = ran_rows(rows).map(|r| (r.suite, r.opt.clone())).collect();
    keys.sort();
    keys.dedup();
    for (s, opt) in keys {
        let sel: Vec<&Row> = ran_rows(rows).filter(|r| r.suite == s && r.opt == opt).collect();
        let cells: Vec<String> = roles
            .iter()
            .map(|&r| sel.iter().filter(|x| x.class(r) == Some(Class::Pass)).count().to_string())
            .collect();
        let _ = writeln!(o, "| {} | {opt} | {} | {} |", s.name(), sel.len(), cells.join(" | "));
    }
    o.push('\n');

    if roles.len() > 1 {
        let _ = writeln!(o, "### kiln against the oracles (cases)\n");
        let kfail = |r: &&Row| r.class(Role::Kiln).is_some_and(Class::is_failure);
        let pass = |r: &Row, role: Role| r.class(role) == Some(Class::Pass);
        let fail = |r: &Row, role: Role| r.class(role).is_some_and(Class::is_failure);
        let oracles: Vec<Role> = roles.iter().copied().filter(|&r| r != Role::Kiln).collect();
        let n =
            ran_rows(rows).filter(kfail).filter(|r| oracles.iter().all(|&x| pass(r, x))).count();
        let _ = writeln!(o, "- kiln fails, every oracle passes: {n}");
        for &x in &oracles {
            let n = ran_rows(rows).filter(kfail).filter(|r| fail(r, x)).count();
            let _ = writeln!(o, "- kiln fails, {} fails too: {n}", x.name());
            let n = ran_rows(rows).filter(|r| pass(r, Role::Kiln) && fail(r, x)).count();
            let _ = writeln!(o, "- kiln passes, {} fails: {n}", x.name());
        }
        let n = ran_rows(rows).filter(kfail).filter(|r| r.xfail).count();
        let _ = writeln!(o, "- kiln failures GCC's testsuite marks xfail: {n}\n");
    }

    let cl = clusters(rows, Role::Kiln);
    if !cl.is_empty() {
        let _ = writeln!(o, "### kiln failure clusters ({} clusters, top 60)\n", cl.len());
        let _ = writeln!(o, "| cases | cluster | examples |");
        let _ = writeln!(o, "|---:|---|---|");
        for (k, v) in cl.iter().take(60) {
            let ex: Vec<&str> = v.iter().take(3).map(String::as_str).collect();
            let _ = writeln!(o, "| {} | {} | {} |", v.len(), md(k), md(&ex.join(", ")));
        }
        o.push('\n');
    }

    let sk = ranked_skips(rows);
    if !sk.is_empty() {
        let _ = writeln!(o, "### Skips (rows; `*` rows cover every option set)\n");
        let _ = writeln!(o, "| rows | reason |");
        let _ = writeln!(o, "|---:|---|");
        for (w, n) in sk.iter().take(40) {
            let _ = writeln!(o, "| {n} | {} |", md(w));
        }
        if sk.len() > 40 {
            let rest: usize = sk[40..].iter().map(|x| x.1).sum();
            let _ = writeln!(o, "| {rest} | ({} other reasons) |", sk.len() - 40);
        }
        o.push('\n');
    }

    let dropped: Vec<String> = info
        .dropped
        .iter()
        .filter(|(_, d)| !d.is_empty())
        .map(|(r, d)| format!("- {}: `{}`", r.name(), d.join("`, `")))
        .collect();
    if !dropped.is_empty() {
        let _ = writeln!(o, "### Flags a compiler rejected (dropped from its cases)\n");
        for d in dropped {
            let _ = writeln!(o, "{d}");
        }
        o.push('\n');
    }
    let _ = writeln!(o, "{end}");
    o
}

/// `existing` with its generated region for `std` (see [`markers`])
/// replaced by `region` (appended when it has none).
pub fn splice(existing: &str, region: &str, std: Option<&str>) -> String {
    let (begin, end) = markers(std);
    let b = existing.find(&begin);
    match (b, b.and_then(|b| existing[b..].find(&end).map(|e| b + e))) {
        (Some(b), Some(e)) => {
            let after = &existing[e + end.len()..];
            let after = after.strip_prefix('\n').unwrap_or(after);
            format!("{}{region}{after}", &existing[..b])
        }
        _ if existing.is_empty() => region.to_owned(),
        _ => format!("{}\n\n{region}", existing.trim_end()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gcc_tests::outcome::Verdict;

    fn args(a: &[&str]) -> Vec<String> {
        a.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn parses_options() {
        let o = parse_args(args(&[
            "--suite",
            "ieee,compile",
            "--opt=-O1,-O3 -g",
            "--std",
            "none",
            "--oracles",
            "clang",
            "-j",
            "2",
        ]))
        .unwrap()
        .unwrap();
        assert_eq!(o.suites, [Suite::Ieee, Suite::Compile]);
        assert_eq!(o.opts, ["-O1", "-O3 -g"]);
        assert_eq!(o.std, None);
        assert_eq!(o.oracles, [Role::Clang]);
        assert_eq!(o.jobs, 2);
        assert!(parse_args(args(&["--help"])).unwrap().is_none());
        assert!(parse_args(args(&["--suite", "bogus"])).is_err());
        assert!(parse_args(args(&["--oracles", "icc"])).is_err());
        assert_eq!(parse_args(args(&["--oracles", "none"])).unwrap().unwrap().oracles, []);
    }

    fn test(suite: Suite) -> Test {
        Test { suite, id: format!("{}/t.c", suite.rel_dir()), sources: vec![PathBuf::from("t.c")] }
    }

    #[test]
    #[allow(clippy::many_single_char_names)]
    fn plans_dg_and_x_suites() {
        let t = Target::for_arch("aarch64", false);
        let opts = args(&["-O0", "-O2"]);
        let text = "/* { dg-skip-if \"\" { *-*-* } { \"-O0\" } { \"\" } } */\n/* { dg-additional-options \"-fwrapv\" } */\n";
        let p = plan_test(&test(Suite::Execute), text, None, &opts, &t);
        assert!(matches!(&p[0], Plan::Skip(_)));
        let Plan::Run(r) = &p[1] else { panic!("{p:?}") };
        assert_eq!(r.flags, ["-w", "-fwrapv"]);
        assert_eq!(r.action, Action::Run);
        let Plan::Run(c) = &plan_test(&test(Suite::Compile), "", None, &opts, &t)[0] else {
            panic!()
        };
        assert_eq!(c.action, Action::Assemble);

        let x = "if [istarget \"aarch64*-*-*\"] { lappend additional_flags \"-ffloat-store\" }\nreturn 0\n";
        let Plan::Run(r) = &plan_test(&test(Suite::Ieee), "", Some(x), &opts, &t)[0] else {
            panic!()
        };
        assert_eq!(r.flags, ["-fno-inline", "-ffloat-store", "-w"]);
        let p = plan_test(&test(Suite::Ieee), "", Some("return 1\n"), &opts, &t);
        assert_eq!(p, vec![Plan::Skip(".x returns 1".into()); 2]);
        let before = "set torture_eval_before_compile {\n  if [string match {*-O2*} \"$option\"] { continue }\n}\nreturn 0\n";
        let p = plan_test(&test(Suite::Ieee), "", Some(before), &opts, &t);
        assert!(matches!(p[0], Plan::Run(_)));
        assert_eq!(p[1], Plan::Skip(".x torture_eval_before_compile".into()));
    }

    #[test]
    fn collapses_whole_test_skips() {
        let t = Target::for_arch("aarch64", false);
        let tests = vec![test(Suite::Ieee), test(Suite::Execute)];
        let bodies = vec![(String::new(), Some("return 1\n".into())), (String::new(), None)];
        let (jobs, skips) = plan_all(&tests, &bodies, &args(&["-O0", "-O2"]), Some("gnu23"), &t);
        assert_eq!(skips.len(), 1);
        assert_eq!(skips[0].opt, "*");
        assert_eq!(jobs.len(), 2);
        assert_eq!(jobs[1].flags, ["-std=gnu23", "-O2", "-w"]);
    }

    fn row(test: &str, k: Class, detail: &str, g: Class) -> Row {
        let ran = |c: Class, d: &str| Ran { verdict: Verdict::new(c, d), dropped: Vec::new() };
        Row {
            test: test.into(),
            suite: Suite::Execute,
            opt: "-O0".into(),
            skip: None,
            xfail: false,
            results: BTreeMap::from([(Role::Kiln, ran(k, detail)), (Role::Gcc, ran(g, ""))]),
        }
    }

    #[test]
    fn reports_and_splices() {
        let rows = vec![
            row("execute/a.c", Class::CompileReject, "error: unknown type name 'x'", Class::Pass),
            row("execute/b.c", Class::CompileReject, "error: unknown type name 'y'", Class::Pass),
            row("execute/c.c", Class::Pass, "", Class::Wrong),
        ];
        let roles = [Role::Kiln, Role::Gcc];
        let cl = clusters(&rows, Role::Kiln);
        assert_eq!(cl.len(), 1);
        assert_eq!(cl[0].1.len(), 2);
        let t = tsv(&rows, &roles);
        assert_eq!(t.lines().count(), 4);
        assert!(t.lines().nth(1).unwrap().starts_with("execute/a.c\t-O0\tcompile-reject\t"));
        assert!(t.lines().nth(1).unwrap().contains("\t-\t\t"), "{t}");
        assert_eq!(summary_line(&rows, &roles), "kiln: 1/3 pass, gcc: 2/3 pass; 0 skip rows");
        let region =
            render(&RunInfo { std: Some("gnu23".into()), ..RunInfo::default() }, &rows, &roles);
        assert!(region.contains("- kiln fails, every oracle passes: 2"));
        assert!(region.contains("- kiln passes, gcc fails: 1"));
        let page = splice(&format!("# T\n\n{BEGIN}\nold\n{END}\ntail\n"), &region, Some("gnu23"));
        assert!(page.starts_with("# T\n\n<!-- gcc-torture:begin") && page.ends_with("tail\n"));
        assert!(!page.contains("old"));
        assert_eq!(splice("", &region, Some("gnu23")), region);
        // WP-1282: a gnu17 run has its own region beside the gnu23 one.
        let g23 = region;
        let g17 =
            render(&RunInfo { std: Some("gnu17".into()), ..RunInfo::default() }, &rows, &roles);
        let page = splice(&splice("# T\n", &g23, Some("gnu23")), &g17, Some("gnu17"));
        assert!(page.contains(BEGIN) && page.contains("<!-- gcc-torture:end std=gnu17 -->"));
        let again = splice(&page, &g17, Some("gnu17"));
        assert_eq!(again, page);
        assert_eq!(splice(&page, &g23, Some("gnu23")), page);
    }

    #[test]
    fn gcc_beside_gxx() {
        assert_eq!(gcc_beside(Path::new("/t/bin/g++-16")), PathBuf::from("/t/bin/gcc-16"));
        assert_eq!(gcc_beside(Path::new("gcc-16")), PathBuf::from("gcc-16"));
    }
}
