//! `cargo xtask gxx-dg`: GCC's C++ testsuite (`g++.dg` with
//! `g++.dg/torture`, `c-c++-common`, `g++.old-deja`) through kiln++, GCC
//! 16.1 and clang, case by case, with each case's diagnostics matched by
//! line and severity against the test's `dg-error` / `dg-warning` lines
//! (spec/harness.md "GCC testsuites", "The g++.dg runner"). Informational:
//! the exit status is non-zero only when the tool itself fails.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use super::dg::{self, Action, Directive, Plan, RunPlan, SuiteDefaults};
use super::expect::{self, Expected};
use super::outcome::{self, Class, Verdict};
use super::target::{Target, cxx_std_of};
use super::{Case, GCC_TAG, Ran, Role, Tool, tcl};
use crate::libstdcxx::glob_match;
use crate::util::process::MemLimit;
use crate::util::{Profile, build_kiln_with, repo_root, target_dir};

const USAGE: &str = "\
usage: cargo xtask gxx-dg [options]

Runs GCC's C++ tests (GCC 16.1's g++.dg, g++.dg/torture, c-c++-common and
g++.old-deja) through kiln++ at the standards GCC's g++-dg.exp picks for
each test, then the failing cases (or every case, --oracles-on all)
through GCC 16.1 and clang. Each case's errors are matched by line and
severity against the test's dg-error / dg-warning / dg-message lines (the
message text is GCC's wording and is never compared), and each case is
classed pass, compile-reject (rejects-valid), accept-invalid, wrong-line,
wrong (a run test failed), crash or timeout per compiler. Writes a per-case
TSV and a Markdown summary. Informational: the exit status is non-zero
only when the tool fails. Contract: spec/harness.md \"GCC testsuites\".

  --tests <dir>          a GCC checkout holding gcc/testsuite (default:
                         .oss/gcc-tests, fetched by scripts/fetch-gcc-tests.sh)
  --suite <list>         comma-separated: dg (g++.dg without torture),
                         torture, common (c-c++-common), old-deja
                         (default: all four)
  --std <list>           gcc (default: g++-dg.exp's per-test list, the
                         versions the test names filled to three from
                         20, 98, 26, 17, 11, 14, 23), or a comma list of
                         years used as GXX_TESTSUITE_STDS is (e.g. 20,26)
  --opt <list>           option sets of the torture suite (default -O0,-O2)
  --profile <p>          the cargo profile to build kiln with (default harness)
  --compiler <path>      kiln's C++ driver (default: the kiln++ cargo builds)
  --gcc <path>           GCC's C++ driver (default: $KILN_GCC, else g++-16)
  --clang <path>         clang's C++ driver (default: clang++-22)
  --oracles <list>       gcc,clang (default), gcc, clang or none
  --oracles-on <when>    fail (default: only cases kiln does not pass) or all
  --filter <globs>       only tests whose id (g++.dg/cpp26/foo.C) matches one
                         of these comma-separated globs
  --sample <n>           a deterministic sample: n tests evenly spaced over
                         the sorted selection
  --shard <K/N>          run only every N-th selected test, starting at the
                         K-th (1-based; the N shards are disjoint and cover
                         the selection; a test's cases stay together)
  --list                 list the planned cases and skips, run nothing
  -j, --jobs <n>         cases in parallel (default 4; an upper bound under a
                         MAKEFLAGS jobserver, see spec/harness.md)
  --timeout <secs>       per compile or link step (default 60)
  --run-timeout <secs>   per test run, times dg-timeout-factor (default 10)
  --mem-limit <MiB>      address-space cap per step, plus kiln's stack
                         reservation (default 2048; 0: none)
  --tsv <file>           the per-case TSV (default: <target>/xtask-gxx-dg/results.tsv)
  --summary <file.md>    the summary (default: <target>/xtask-gxx-dg/summary.md)
  --report <file.md>     also splice the summary into this page
  -v, --verbose          print every case's result
";

/// A part of the C++ testsuite and the `.exp` driver that runs it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Suite {
    /// `g++.dg/**/*.C` outside directories with their own `.exp`
    /// (`g++.dg/dg.exp`): g++-dg-runtest, `-pedantic-errors -Wno-long-long`.
    Dg,
    /// `g++.dg/torture/*.C` and `c-c++-common/torture/*.c`
    /// (`dg-torture.exp`): gcc-dg-runtest over the option sets, no `-std`.
    Torture,
    /// `c-c++-common/*.[cSi]` and `c-c++-common/cpp/*.[cS]`, compiled as
    /// C++ (`g++.dg/dg.exp`): g++-dg-runtest, no default flags.
    Common,
    /// `g++.old-deja/**/*.C` (`old-deja.exp`): g++-dg-runtest,
    /// `-pedantic-errors -Wno-long-long`.
    OldDeja,
}

impl Suite {
    pub const ALL: [Self; 4] = [Self::Dg, Self::Torture, Self::Common, Self::OldDeja];

    pub fn name(self) -> &'static str {
        match self {
            Self::Dg => "dg",
            Self::Torture => "torture",
            Self::Common => "common",
            Self::OldDeja => "old-deja",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|x| x.name() == s)
    }

    /// The `default-extra-flags` its driver passes (`dg-options` replaces them).
    pub fn default_flags(self) -> &'static [&'static str] {
        match self {
            Self::Dg | Self::OldDeja => &["-pedantic-errors", "-Wno-long-long"],
            Self::Torture | Self::Common => &[],
        }
    }
}

/// GCC 16's default C++ dialect (`cxx_default` in target-supports.exp),
/// passed explicitly when a case has no `-std=` so that kiln and clang
/// compile at GCC's default too.
pub const CXX_DEFAULT: &str = "-std=gnu++20";

#[derive(Clone, Debug, PartialEq)]
pub struct Options {
    pub tests: Option<PathBuf>,
    pub suites: Vec<Suite>,
    /// `None`: GCC's per-test list; else `GXX_TESTSUITE_STDS`.
    pub stds: Option<Vec<String>>,
    pub opts: Vec<String>,
    pub profile: Profile,
    pub compiler: Option<PathBuf>,
    pub gcc: Option<PathBuf>,
    pub clang: PathBuf,
    pub oracles: Vec<Role>,
    pub oracles_always: bool,
    pub filter: Option<String>,
    pub sample: Option<usize>,
    /// `--shard K/N`: (K, N), 1-based; keep every N-th selected test.
    pub shard: Option<(usize, usize)>,
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
            suites: Suite::ALL.to_vec(),
            stds: None,
            opts: vec!["-O0".into(), "-O2".into()],
            profile: Profile::Harness,
            compiler: None,
            gcc: None,
            clang: PathBuf::from("clang++-22"),
            oracles: vec![Role::Gcc, Role::Clang],
            oracles_always: false,
            filter: None,
            sample: None,
            shard: None,
            list: false,
            jobs: 4,
            timeout: Duration::from_secs(60),
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
#[allow(clippy::too_many_lines, reason = "one flag table")]
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
            "--std" => {
                let v = value("--std")?;
                o.stds = if v == "gcc" {
                    None
                } else {
                    let l = comma_list(&v);
                    if let Some(bad) = l.iter().find(|y| super::target::cxx_std_number(y).is_none())
                    {
                        return Err(format!("bad --std year `{bad}` (98, 11, ... 26, or gcc)"));
                    }
                    Some(l)
                };
            }
            "--opt" => {
                o.opts = comma_list(&value("--opt")?);
                if o.opts.is_empty() {
                    return Err("`--opt` names no option set".into());
                }
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
            "--oracles-on" => {
                o.oracles_always = match value("--oracles-on")?.as_str() {
                    "all" => true,
                    "fail" => false,
                    v => return Err(format!("bad --oracles-on `{v}` (fail or all)")),
                };
            }
            "--filter" => o.filter = Some(value("--filter")?),
            "--sample" => {
                let v = value("--sample")?;
                o.sample = Some(
                    v.parse()
                        .ok()
                        .filter(|&n| n > 0)
                        .ok_or_else(|| format!("bad --sample `{v}`"))?,
                );
            }
            "--shard" => o.shard = Some(crate::libcxx::parse_shard(&value("--shard")?)?),
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
            eprintln!("xtask gxx-dg: {e}\n\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    match execute(&opts) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("xtask gxx-dg: {e}");
            ExitCode::from(2)
        }
    }
}

/// One test: its id (its path under `gcc/testsuite`) and file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Test {
    pub suite: Suite,
    pub id: String,
    pub path: PathBuf,
}

impl Test {
    /// The directory a report groups the test under: `g++.dg/cpp26`,
    /// `g++.dg` for a top-level test, `c-c++-common`, `g++.old-deja/g++.pt`.
    pub fn group(&self) -> String {
        group_of(&self.id)
    }
}

/// See [`Test::group`].
pub fn group_of(id: &str) -> String {
    let parts: Vec<&str> = id.split('/').collect();
    match parts.as_slice() {
        [top, _file] => (*top).to_owned(),
        [top, sub, ..] => format!("{top}/{sub}"),
        _ => id.to_owned(),
    }
}

fn files(dir: &Path, exts: &[&str]) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|rd| {
            rd.filter_map(Result::ok)
                .map(|e| e.path())
                .filter(|p| p.is_file())
                .filter(|p| {
                    p.extension().and_then(|e| e.to_str()).is_some_and(|e| exts.contains(&e))
                })
                .collect()
        })
        .unwrap_or_default();
    v.sort();
    v
}

fn subdirs(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|rd| rd.filter_map(Result::ok).map(|e| e.path()).filter(|p| p.is_dir()).collect())
        .unwrap_or_default();
    v.sort();
    v
}

/// `find-cxx-tests` of `g++.dg/dg.exp`: the `*.C` of `dir` and of every
/// subdirectory without an `.exp` file, recursively. `own` collects the
/// directories skipped for having one, with their test counts.
fn find_cxx_tests(dir: &Path, own: &mut Vec<(PathBuf, usize)>) -> Vec<PathBuf> {
    let mut out = files(dir, &["C"]);
    for sub in subdirs(dir) {
        if files(&sub, &["exp"]).is_empty() {
            out.extend(find_cxx_tests(&sub, own));
        } else {
            own.push((sub.clone(), count_recursive(&sub)));
        }
    }
    out
}

fn count_recursive(dir: &Path) -> usize {
    files(dir, &["C", "c", "cc"]).len()
        + subdirs(dir).iter().map(|d| count_recursive(d)).sum::<usize>()
}

/// `find $dir *.C` (old-deja.exp): every `*.C` below `dir`.
fn find_all(dir: &Path) -> Vec<PathBuf> {
    let mut out = files(dir, &["C"]);
    for sub in subdirs(dir) {
        out.extend(find_all(&sub));
    }
    out
}

/// The tests of `suite` under `ts` (`gcc/testsuite`), in its driver's
/// order, and the directories of g++.dg its own `.exp` drives (not run).
pub fn discover(ts: &Path, suite: Suite) -> (Vec<Test>, Vec<(String, usize)>) {
    let mut own = Vec::new();
    let paths: Vec<PathBuf> = match suite {
        Suite::Dg => {
            let root = ts.join("g++.dg");
            let mut out = files(&root, &["C"]);
            for sub in subdirs(&root) {
                if sub.file_name().is_some_and(|n| n == "torture") {
                    continue;
                }
                if files(&sub, &["exp"]).is_empty() {
                    out.extend(find_cxx_tests(&sub, &mut own));
                } else {
                    own.push((sub.clone(), count_recursive(&sub)));
                }
            }
            out
        }
        Suite::Torture => {
            let t = ts.join("g++.dg").join("torture");
            for sub in subdirs(&t) {
                own.push((sub.clone(), count_recursive(&sub)));
            }
            let mut v = files(&t, &["C"]);
            v.extend(files(&ts.join("c-c++-common").join("torture"), &["c"]));
            v
        }
        Suite::Common => {
            let c = ts.join("c-c++-common");
            let mut v = files(&c, &["c", "S", "i"]);
            v.extend(files(&c.join("cpp"), &["c", "S"]));
            v
        }
        Suite::OldDeja => find_all(&ts.join("g++.old-deja")),
    };
    let rel = |p: &Path| {
        p.strip_prefix(ts).map_or_else(|_| p.display().to_string(), |r| r.display().to_string())
    };
    let tests = paths.iter().map(|p| Test { suite, id: rel(p), path: p.clone() }).collect();
    let own = own.iter().map(|(p, n)| (rel(p), *n)).collect();
    (tests, own)
}

/// `g++-std-flags` of GCC 16's `g++-dg.exp`: the `-std=` flag of each run
/// of a test (one empty entry when the test names a `-std=*++` itself).
/// `stds` is `gpp_std_list` (`GXX_TESTSUITE_STDS`), empty for the default.
pub fn std_flags(body: &str, id: &str, stds: &[String]) -> Vec<String> {
    let lines: Vec<&str> = body.lines().collect();
    if lines.iter().any(|l| tcl::glob("*-std=*++*", l)) {
        return vec![String::new()];
    }
    let prefix =
        if lines.iter().any(|l| l.contains("dg-options")) { "-std=gnu++" } else { "-std=c++" };
    let mut low = 0u32;
    if tcl::glob("*/coroutines/*", id) {
        low = 20;
    }
    if tcl::glob("*/modules/*", id) {
        low = 17;
    }
    let cmp = |v: &str| -> u32 {
        let v = if v == "98" { "03" } else { v };
        v.parse().unwrap_or(0)
    };
    let mut list: Vec<String> = Vec::new();
    if stds.is_empty() {
        for l in lines.iter().filter(|l| tcl::glob("*\\{ dg* c++[0-9][0-9]*", l)) {
            let Some(ver) = first_cxx_version(l) else { continue };
            if ver != "98" && (l.contains("dg-do") || l.contains("dg-require-effective-target")) {
                low = cmp(&ver);
            }
            list.push(ver);
        }
        list.sort();
        list.dedup();
        for ver in ["20", "98", "26", "17", "11", "14", "23"] {
            if list.len() < 3 && !list.iter().any(|x| x == ver) && cmp(ver) >= low {
                list.push(ver.to_owned());
            }
        }
    } else {
        for ver in stds {
            if !list.contains(ver) && cmp(ver) >= low {
                list.push(ver.clone());
            }
        }
    }
    list.iter().map(|v| format!("{prefix}{v}")).collect()
}

/// The two digits after the first `c++` followed by two digits on a line.
fn first_cxx_version(line: &str) -> Option<String> {
    let b = line.as_bytes();
    let mut from = 0;
    while let Some(i) = line[from..].find("c++") {
        let at = from + i + 3;
        if b.len() >= at + 2 && b[at].is_ascii_digit() && b[at + 1].is_ascii_digit() {
            return Some(line[at..at + 2].to_owned());
        }
        from = at;
    }
    None
}

/// One case of a test: the compile flags and what it expects.
#[derive(Clone, Debug, PartialEq)]
pub struct CasePlan {
    /// The `-std=` (or nothing) and torture option set, as the label.
    pub opt: String,
    pub plan: RunPlan,
    /// Every compile flag: the `-std=` (or [`CXX_DEFAULT`]), the option
    /// set, then the test's flags.
    pub flags: Vec<String>,
    pub expected: Expected,
    pub sources: Vec<PathBuf>,
    /// `dg-shouldfail`: the program is expected to fail at run time.
    pub shouldfail: bool,
}

/// A test's GCC-internal checks (`dg-final scan-*` and the like): never
/// compared, counted per kind.
pub fn unchecked(dirs: &[Directive]) -> Vec<String> {
    let mut v: Vec<String> = Vec::new();
    for d in dirs {
        let kind = match d.name.as_str() {
            "dg-final" => {
                let inner = d.args.first().map_or("", tcl::Word::text);
                tcl::list(inner).first().map_or("dg-final", |w| w.text()).to_owned()
            }
            "dg-output" | "dg-output-file" => d.name.clone(),
            _ => continue,
        };
        if !v.contains(&kind) {
            v.push(kind);
        }
    }
    v
}

/// The skip that applies to a whole test before its directives are
/// resolved: input kinds the runner does not compile and suites it counts
/// but does not run (modules).
fn whole_test_skip(test: &Test, dirs: &[Directive]) -> Option<String> {
    let ext = test.path.extension().and_then(|e| e.to_str()).unwrap_or("");
    if matches!(ext, "S" | "i") {
        return Some(format!("input kind .{ext} (assembler or preprocessed)"));
    }
    if dirs.iter().any(|d| d.name.starts_with("dg-module-")) {
        return Some("modules (dg-module-*)".into());
    }
    None
}

/// The `gcc/testsuite` directory a test sits under (`$srcdir`).
fn srcdir_of(test: &Test) -> String {
    let p = test.path.display().to_string();
    p.strip_suffix(&test.id).map_or_else(|| ".".to_owned(), |s| s.trim_end_matches('/').to_owned())
}

/// `$subdir`: the directory of the `.exp` driver that runs the test,
/// relative to `$srcdir` (`g++.dg/dg.exp` runs `dg` and `common`).
fn subdir_of(suite: Suite) -> &'static str {
    match suite {
        Suite::Dg | Suite::Common => "g++.dg",
        Suite::Torture => "g++.dg/torture",
        Suite::OldDeja => "g++.old-deja",
    }
}

/// `${srcdir}` / `$srcdir` and `${subdir}` / `$subdir` in a flag replaced
/// as the Tcl evaluation of the directive replaces them: the testsuite
/// directory and the driver's directory under it.
fn with_srcdir(flag: &str, srcdir: &str, subdir: &str) -> String {
    flag.replace("${srcdir}", srcdir)
        .replace("$srcdir", srcdir)
        .replace("${subdir}", subdir)
        .replace("$subdir", subdir)
}

/// Whether a word of a test's options is an input file the driver
/// compiles and links with the test (`dg-options "-O2 ${srcdir}/x/y.C"`):
/// a unit of its own (not the value of `-include` and the like), not a
/// flag, with a C, C++ or object file's extension.
fn input_file(unit: &[String]) -> Option<&str> {
    let [w] = unit else { return None };
    let ext = Path::new(w).extension()?.to_str()?;
    (!w.starts_with('-') && matches!(ext, "c" | "C" | "cc" | "cpp" | "cxx" | "c++" | "o"))
        .then_some(w.as_str())
}

/// The sources `dg-additional-sources` adds at `target`
/// (`lib/gcc-defs.exp`): each directive's words, relative to the test's
/// directory, under its `{ target <sel> }`; one marked `linkonly` only
/// when the case links. `Err` for a selector the runner cannot read.
fn additional_sources(
    dirs: &[Directive],
    dir: &Path,
    target: &Target,
    links: bool,
) -> Result<Vec<PathBuf>, String> {
    let mut out = Vec::new();
    for d in dirs.iter().filter(|d| d.name == "dg-additional-sources") {
        if let Some(sel) = d.args.get(1) {
            let words = tcl::list(sel.text());
            if words.first().is_some_and(|w| w.text() == "linkonly") {
                if words.len() > 1 {
                    return Err("dg-additional-sources: a selector after linkonly".into());
                }
                if !links {
                    continue;
                }
            } else {
                match target.keyword_selector(sel.text()) {
                    Some((kw, Ok(m))) if kw == "target" => {
                        if !m {
                            continue;
                        }
                    }
                    Some((_, Err(e))) => return Err(format!("dg-additional-sources: {e}")),
                    _ => {
                        return Err(format!("dg-additional-sources: selector `{}`", sel.text()));
                    }
                }
            }
        }
        for f in dg::split_flags(d.args.first().map_or("", tcl::Word::text)) {
            out.push(dir.join(f));
        }
    }
    Ok(out)
}

/// A test's cases (or skips), one per standard and option set.
pub fn plan_test(
    test: &Test,
    body: &str,
    stds: &[String],
    opts: &[String],
    t: &Target,
) -> Vec<(String, Result<CasePlan, String>)> {
    let dirs = dg::directives(body);
    if let Some(why) = whole_test_skip(test, &dirs) {
        return vec![("*".into(), Err(why))];
    }
    let defaults = SuiteDefaults {
        action: Action::Compile,
        extra_flags: test.suite.default_flags().iter().map(|&s| s.to_owned()).collect(),
    };
    // The -std= a case is at: the run's, else the last one in the test's
    // own options (whatever their selectors), else GCC's default.
    let own: Vec<String> = dirs
        .iter()
        .filter(|d| matches!(d.name.as_str(), "dg-options" | "dg-additional-options"))
        .flat_map(|d| dg::split_flags(d.args.first().map_or("", tcl::Word::text)))
        .collect();
    let runs: Vec<(String, Vec<String>)> = if test.suite == Suite::Torture {
        opts.iter().map(|o| (o.clone(), dg::split_flags(o))).collect()
    } else {
        std_flags(body, &test.id, stds)
            .into_iter()
            .map(|s| (s.clone(), dg::split_flags(&s)))
            .collect()
    };
    let shouldfail = dirs.iter().any(|d| d.name == "dg-shouldfail");
    let ice = dirs.iter().any(|d| d.name == "dg-ice");
    let dir = test.path.parent().unwrap_or(Path::new("."));
    let srcdir = srcdir_of(test);
    let subdir = subdir_of(test.suite);
    let mut out = Vec::new();
    for (label, run) in runs {
        let mut lead = run.clone();
        let std = cxx_std_of(&run).or_else(|| cxx_std_of(&own));
        if std.is_none() {
            lead.insert(0, CXX_DEFAULT.to_owned());
        }
        let target = t.with_cxx(std.unwrap_or(20));
        let label = if label.is_empty() { "-".to_owned() } else { label };
        let r = match dg::resolve(&dirs, &defaults, &target, &lead) {
            Plan::Skip(w) => Err(w),
            Plan::Run(mut plan) => {
                let t = target.with_option(&lead);
                match expect::expectations(&dirs, &t)
                    .and_then(|e| Ok((e, additional_sources(&dirs, dir, &t, plan.action.links())?)))
                {
                    Err(e) => Err(e),
                    Ok((mut expected, extra)) => {
                        expected.links = plan.action.links();
                        plan.negative = expected.negative();
                        plan.xfail |= ice;
                        // Input files among the options join the sources,
                        // after the test (the driver's order).
                        let mut sources = vec![test.path.clone()];
                        let mut flags = lead.clone();
                        let own: Vec<String> =
                            plan.flags.iter().map(|f| with_srcdir(f, &srcdir, subdir)).collect();
                        for unit in super::flag_units(&own) {
                            match input_file(&unit) {
                                Some(f) => sources.push(dir.join(f)),
                                None => flags.extend(unit),
                            }
                        }
                        sources.extend(extra);
                        Ok(CasePlan {
                            opt: label.clone(),
                            plan,
                            flags,
                            expected,
                            sources,
                            shouldfail,
                        })
                    }
                }
            }
        };
        out.push((label, r));
    }
    out
}

/// A case's row: the run's results, or a skip (`opt` `*` when the test
/// skips at every case for the same reason).
#[derive(Clone, Debug)]
pub struct Row {
    pub test: String,
    pub group: String,
    pub opt: String,
    pub skip: Option<String>,
    pub xfail: bool,
    pub negative: bool,
    pub results: BTreeMap<Role, Ran>,
}

impl Row {
    pub fn class(&self, r: Role) -> Option<Class> {
        self.results.get(&r).map(|x| x.verdict.class)
    }
}

/// One scheduled case.
#[derive(Clone, Debug)]
pub struct Job {
    pub test: usize,
    pub case: CasePlan,
}

/// The jobs and skip rows of the selected tests, and the unchecked
/// GCC-internal checks per kind (tests).
pub fn plan_all(
    tests: &[Test],
    bodies: &[String],
    stds: &[String],
    opts: &[String],
    t: &Target,
) -> (Vec<Job>, Vec<Row>, BTreeMap<String, usize>) {
    let mut jobs = Vec::new();
    let mut skips = Vec::new();
    let mut unchecked_n: BTreeMap<String, usize> = BTreeMap::new();
    for (i, (test, body)) in tests.iter().zip(bodies).enumerate() {
        let skip_row = |opt: &str, why: &str| Row {
            test: test.id.clone(),
            group: test.group(),
            opt: opt.to_owned(),
            skip: Some(why.to_owned()),
            xfail: false,
            negative: false,
            results: BTreeMap::new(),
        };
        let mut cases = plan_test(test, body, stds, opts, t);
        // A source the test names that is not there (DejaGnu's compile
        // would fail for every compiler): counted, not run.
        for (_, c) in &mut cases {
            if let Ok(case) = c
                && let Some(m) = case.sources.iter().find(|p| !p.is_file())
            {
                let name = m
                    .file_name()
                    .map_or_else(|| m.display().to_string(), |n| n.to_string_lossy().into_owned());
                *c = Err(format!("missing source {name}"));
            }
        }
        let first_skip = match cases.first() {
            Some((_, Err(w))) => Some(w.clone()),
            _ => None,
        };
        if let Some(w) = first_skip
            && cases.iter().all(|(_, c)| matches!(c, Err(x) if *x == w))
        {
            skips.push(skip_row("*", &w));
            continue;
        }
        let mut ran = false;
        for (opt, c) in cases {
            match c {
                Err(w) => skips.push(skip_row(&opt, &w)),
                Ok(case) => {
                    ran = true;
                    jobs.push(Job { test: i, case });
                }
            }
        }
        if ran {
            for k in unchecked(&dg::directives(body)) {
                *unchecked_n.entry(k).or_default() += 1;
            }
        }
    }
    (jobs, skips, unchecked_n)
}

struct Config {
    tools: Vec<Tool>,
    scratch: PathBuf,
    timeout: Duration,
    run_timeout: Duration,
    mem: Option<MemLimit>,
    oracles_always: bool,
    /// Link flags after [`LINK_FLAGS`]: the GCC oracle's libstdc++
    /// run path, when kiln and clang build against it.
    link: Vec<String>,
}

const LINK_FLAGS: &[&str] = &["-lm"];

/// How the report header shows `--gcc-toolchain=<the GCC oracle's prefix>`.
const TOOLCHAIN_NOTE: &str = "--gcc-toolchain=<GCC oracle's prefix>";

/// The flag that lifts a compiler's error limit, led before every command
/// line: GCC has none, so an expected `dg-error` after a clang-style
/// compiler's 20th error would be lost (`-ferror-limit=0` for kiln and
/// clang; nothing for GCC).
fn unlimited_errors(role: Role) -> Option<&'static str> {
    match role {
        Role::Kiln | Role::Clang => Some("-ferror-limit=0"),
        Role::Gcc => None,
    }
}

/// A `dg-shouldfail` case's verdict from its plain one: a failing run is
/// its pass, a clean run is `wrong`.
fn should_fail(v: Verdict) -> Verdict {
    match v.class {
        Class::Wrong => Verdict::pass(),
        Class::Pass => Verdict::new(Class::Wrong, "ran clean (dg-shouldfail)"),
        _ => v,
    }
}

fn run_job(cfg: &Config, tests: &[Test], job: &Job, idx: usize) -> Row {
    let test = &tests[job.test];
    let c = &job.case;
    let link: Vec<String> =
        LINK_FLAGS.iter().map(|&s| s.to_owned()).chain(cfg.link.iter().cloned()).collect();
    let is_c = test.path.extension().is_some_and(|e| e == "c");
    let case = Case {
        sources: &c.sources,
        action: c.plan.action,
        flags: &c.flags,
        link_flags: &link,
        negative: c.plan.negative,
        compile_timeout: cfg.timeout.mul_f64(c.plan.timeout_factor.max(1.0)),
        run_timeout: cfg.run_timeout.mul_f64(c.plan.timeout_factor.max(1.0)),
        mem: cfg.mem,
        lang: is_c.then_some("c++"),
        expect: Some(&c.expected),
    };
    let dir = cfg.scratch.join(idx.to_string());
    let mut results = BTreeMap::new();
    for tool in &cfg.tools {
        if tool.role != Role::Kiln
            && !cfg.oracles_always
            && results.get(&Role::Kiln).is_some_and(|r: &Ran| r.verdict.class == Class::Pass)
        {
            continue;
        }
        let mut ran = tool.run_case(&case, &dir.join(tool.role.name()));
        if c.shouldfail && c.plan.action == Action::Run {
            ran.verdict = should_fail(ran.verdict);
        }
        results.insert(tool.role, ran);
    }
    let _ = std::fs::remove_dir_all(&dir);
    Row {
        test: test.id.clone(),
        group: test.group(),
        opt: c.opt.clone(),
        skip: None,
        xfail: c.plan.xfail,
        negative: c.plan.negative,
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

/// Runs every job on `n` workers. Each finished row is also appended to
/// `partial` (the TSV's rows, in completion order), so an interrupted run
/// keeps what it measured.
fn run_all(
    cfg: &Config,
    tests: &[Test],
    jobs: &[Job],
    n: usize,
    verbose: bool,
    partial: &Path,
) -> Vec<Row> {
    use std::io::Write as _;
    let sink = Mutex::new(std::fs::File::create(partial).ok());
    if let Some(f) = sink.lock().expect("not poisoned yet").as_mut() {
        let _ = f.write_all(tsv(&[]).as_bytes());
    }
    let next = AtomicUsize::new(0);
    let done = AtomicUsize::new(0);
    let slots: Mutex<Vec<Option<Row>>> = Mutex::new(vec![None; jobs.len()]);
    let start = Instant::now();
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
                    } else if finished.is_multiple_of(500) {
                        println!(
                            "[{finished}/{}] {:.0} s",
                            jobs.len(),
                            start.elapsed().as_secs_f64()
                        );
                    }
                    if let Some(f) = sink.lock().expect("no panics under the lock").as_mut() {
                        let line = tsv(std::slice::from_ref(&r));
                        let _ = f.write_all(line.split_once('\n').map_or("", |x| x.1).as_bytes());
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

/// The compilers, and the link flags that go with them. When the GCC
/// oracle runs, kiln and clang build against its libstdc++
/// (`--gcc-toolchain=`, the headers GCC's testsuite was written for,
/// `<meta>` among them) and every executable gets its library directory
/// as run path, as the GCC install gives its own.
fn resolve_tools(
    opts: &Options,
    root: &Path,
    scratch: &Path,
) -> Result<(Vec<Tool>, Vec<String>), String> {
    let llvm = crate::oracle::Oracle::from_env().unwrap_or_default().llvm_dir();
    let kiln_env = vec![("KILN_LLVM_DIR".to_owned(), llvm.display().to_string())];
    let kiln = if let Some(c) = &opts.compiler {
        c.clone()
    } else {
        let target = target_dir(root)?;
        build_kiln_with(root, &target, "gxx-dg", opts.profile)?
    };
    let mut tools = vec![Tool::new_for(Role::Kiln, kiln, kiln_env, scratch, "probe.cc")?];
    if let Some(f) = unlimited_errors(Role::Kiln) {
        tools[0].lead_with(f.to_owned());
    }
    let mut toolchain = None;
    if opts.oracles.contains(&Role::Gcc) {
        let (gxx, explicit) = if let Some(g) = &opts.gcc {
            (g.clone(), true)
        } else {
            let env = std::env::var(crate::oracle::GCC_ENV).ok();
            crate::oracle::gcc_from(env.as_deref())
        };
        // As GCC's testsuite runs it (`TEST_ALWAYS_FLAGS` in prune.exp,
        // `LC_ALL=C` in gcc-dg.exp): one located line per diagnostic, no
        // nested `•` bullets without a location.
        let env = vec![("LC_ALL".to_owned(), "C".to_owned())];
        match Tool::new_for(Role::Gcc, gxx.clone(), env, scratch, "probe.cc") {
            Ok(mut t) => {
                t.lead_with("-fdiagnostics-plain-output".to_owned());
                tools.push(t);
                toolchain = crate::oracle::gcc_toolchain_of(&gxx);
            }
            Err(e) if explicit => return Err(e),
            Err(e) => eprintln!("xtask gxx-dg: no GCC oracle ({e}); set KILN_GCC"),
        }
    }
    if opts.oracles.contains(&Role::Clang) {
        let mut clang =
            Tool::new_for(Role::Clang, opts.clang.clone(), Vec::new(), scratch, "probe.cc")?;
        if let Some(f) = unlimited_errors(Role::Clang) {
            clang.lead_with(f.to_owned());
        }
        if let Some(tc) = &toolchain {
            clang.lead_noted(format!("--gcc-toolchain={}", tc.display()), TOOLCHAIN_NOTE);
        }
        if let Some(m) = clang.clang_major().filter(|m| crate::oracle::SUPPORTED.contains(m)) {
            tools[0].lead_with(format!("-fclang-compat={m}"));
        }
        tools.push(clang);
    }
    let mut link = Vec::new();
    if let Some(tc) = &toolchain {
        tools[0].lead_noted(format!("--gcc-toolchain={}", tc.display()), TOOLCHAIN_NOTE);
        let lib =
            ["lib64", "lib"].iter().map(|d| tc.join(d)).find(|d| d.join("libstdc++.so").exists());
        if let Some(lib) = lib {
            link.push(format!("-Wl,-rpath,{}", lib.display()));
        }
    }
    Ok((tools, link))
}

/// `n` tests evenly spaced over `tests` (all of them when `n` is not
/// smaller), keeping their order.
pub fn sample<T: Clone>(tests: &[T], n: usize) -> Vec<T> {
    if n >= tests.len() {
        return tests.to_vec();
    }
    (0..n).map(|i| tests[i * tests.len() / n].clone()).collect()
}

#[allow(clippy::too_many_lines, reason = "one linear plan / run / report sequence")]
fn execute(opts: &Options) -> Result<(), String> {
    let start = Instant::now();
    let root = repo_root();
    let testsuite = super::locate(&root, opts.tests.as_deref())?;
    if !testsuite.join("g++.dg").is_dir() {
        return Err(format!(
            "no g++.dg under {} (rerun scripts/fetch-gcc-tests.sh)",
            testsuite.display()
        ));
    }
    let target = Target::host();
    let mut tests = Vec::new();
    let mut own = Vec::new();
    for &s in &opts.suites {
        let (t, o) = discover(&testsuite, s);
        tests.extend(t);
        own.extend(o);
    }
    if let Some(f) = &opts.filter {
        let globs = comma_list(f);
        tests.retain(|t| globs.iter().any(|g| glob_match(g, &t.id)));
    }
    if let Some(n) = opts.sample {
        tests = sample(&tests, n);
    }
    if let Some(shard) = opts.shard {
        let all = tests.len();
        tests = crate::libcxx::shard_of(tests, shard);
        println!("xtask gxx-dg: shard {}/{}: {} of {all} tests", shard.0, shard.1, tests.len());
    }
    let bodies: Vec<String> = tests
        .iter()
        .map(|t| {
            std::fs::read(&t.path)
                .map(|b| String::from_utf8_lossy(&b).into_owned())
                .unwrap_or_default()
        })
        .collect();
    let stds = opts.stds.clone().unwrap_or_default();
    let (jobs, skips, unchecked_n) = plan_all(&tests, &bodies, &stds, &opts.opts, &target);
    if opts.list {
        for j in &jobs {
            println!(
                "{} [{}] {} {}",
                tests[j.test].id,
                j.case.opt,
                j.case.plan.action.name(),
                j.case.flags.join(" ")
            );
        }
        for s in &skips {
            println!("{} [{}] skip: {}", s.test, s.opt, s.skip.as_deref().unwrap_or(""));
        }
        for (why, n) in ranked_skips(&skips) {
            println!("skip {n:>5}  {why}");
        }
        for (d, n) in &own {
            println!("own-exp {n:>5}  {d}");
        }
        println!("{} tests, {} cases, {} skip rows", tests.len(), jobs.len(), skips.len());
        return Ok(());
    }

    let tdir = target_dir(&root)?;
    let out_dir = tdir.join("xtask-gxx-dg");
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.subsec_nanos());
    let scratch = out_dir.join(format!("run-{}-{nanos}", std::process::id()));
    std::fs::create_dir_all(&scratch)
        .map_err(|e| format!("cannot create {}: {e}", scratch.display()))?;
    let (tools, link) = resolve_tools(opts, &root, &scratch)?;
    let cfg = Config {
        tools,
        scratch: scratch.clone(),
        timeout: opts.timeout,
        run_timeout: opts.run_timeout,
        mem: MemLimit::from_mib(opts.mem_limit_mib),
        oracles_always: opts.oracles_always,
        link,
    };
    println!(
        "xtask gxx-dg: {} cases from {} tests ({} skip rows) under {}, compilers {} (-j {}{})",
        jobs.len(),
        tests.len(),
        skips.len(),
        testsuite.display(),
        cfg.tools.iter().map(|t| t.path.display().to_string()).collect::<Vec<_>>().join(", "),
        opts.jobs,
        crate::jobs::global().describe().map_or_else(String::new, |d| format!(", jobserver {d}")),
    );
    let tsv_path = opts.tsv.clone().unwrap_or_else(|| out_dir.join("results.tsv"));
    let md_path = opts.summary.clone().unwrap_or_else(|| out_dir.join("summary.md"));
    for p in [&tsv_path, &md_path] {
        if let Some(d) = p.parent() {
            let _ = std::fs::create_dir_all(d);
        }
    }
    let partial = tsv_path.with_extension("partial.tsv");
    let mut rows = run_all(&cfg, &tests, &jobs, opts.jobs, opts.verbose, &partial);
    let _ = std::fs::remove_file(&partial);
    let _ = std::fs::remove_dir_all(&scratch);
    rows.extend(skips);
    rows.sort_by(|a, b| (&a.test, &a.opt).cmp(&(&b.test, &b.opt)));

    let roles: Vec<Role> = cfg.tools.iter().map(|t| t.role).collect();
    let info = RunInfo {
        tests_commit: git_out(&testsuite, &["rev-parse", "HEAD"]),
        kiln_commit: git_out(&root, &["rev-parse", "--short", "HEAD"]),
        command: command_line(opts),
        compilers: cfg.tools.iter().map(|t| (t.role, t.version.clone())).collect(),
        dropped: cfg.tools.iter().map(|t| (t.role, t.rejected())).collect(),
        triplet: target.triplet.clone(),
        tests: tests.len(),
        own,
        unchecked: unchecked_n,
        oracles_always: opts.oracles_always,
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
    std::fs::write(&tsv_path, tsv(&rows))
        .map_err(|e| format!("cannot write {}: {e}", tsv_path.display()))?;
    let region = render(&info, &rows, &roles);
    std::fs::write(&md_path, &region)
        .map_err(|e| format!("cannot write {}: {e}", md_path.display()))?;
    println!("\n{}", summary_line(&rows));
    println!("xtask gxx-dg: wrote {} and {}", tsv_path.display(), md_path.display());
    if let Some(path) = &opts.report {
        let existing = std::fs::read_to_string(path).unwrap_or_default();
        std::fs::write(path, super::splice_region(&existing, &region, BEGIN, END))
            .map_err(|e| format!("cannot write {}: {e}", path.display()))?;
        println!("xtask gxx-dg: wrote {}", path.display());
    }
    Ok(())
}

fn git_out(dir: &Path, args: &[&str]) -> String {
    std::process::Command::new("git")
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

fn command_line(o: &Options) -> String {
    let mut c = format!("cargo xtask gxx-dg -j {}", o.jobs);
    let suites: Vec<&str> = o.suites.iter().map(|s| s.name()).collect();
    let _ = write!(c, " --suite {}", suites.join(","));
    if let Some(s) = &o.stds {
        let _ = write!(c, " --std {}", s.join(","));
    }
    if o.suites.contains(&Suite::Torture) {
        let _ = write!(c, " --opt {}", o.opts.join(","));
    }
    if o.oracles_always {
        c.push_str(" --oracles-on all");
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
    if let Some(n) = o.sample {
        let _ = write!(c, " --sample {n}");
    }
    if let Some((k, n)) = o.shard {
        let _ = write!(c, " --shard {k}/{n}");
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
    pub triplet: String,
    pub tests: usize,
    /// Directories their own `.exp` drives (not run), with test counts.
    pub own: Vec<(String, usize)>,
    /// GCC-internal checks of the tests run, per kind (tests).
    pub unchecked: BTreeMap<String, usize>,
    pub oracles_always: bool,
    pub limits: String,
    pub wall: Duration,
}

fn cell(s: &str) -> String {
    s.replace(['\t', '\n', '\r'], " ")
}

/// The per-case TSV: one row per (test, case), skips included.
pub fn tsv(rows: &[Row]) -> String {
    let mut o = String::from("test\topt");
    for r in [Role::Kiln, Role::Gcc, Role::Clang] {
        let _ = write!(o, "\t{0}\t{0}_detail", r.name());
    }
    o.push_str("\tnegative\txfail\tdropped\n");
    for row in rows {
        let _ = write!(o, "{}\t{}", cell(&row.test), cell(&row.opt));
        for r in [Role::Kiln, Role::Gcc, Role::Clang] {
            match (&row.skip, row.results.get(&r)) {
                (Some(why), _) if r == Role::Kiln => {
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
            "\t{}\t{}\t{}",
            if row.negative { "negative" } else { "" },
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

fn ran_rows(rows: &[Row]) -> impl Iterator<Item = &Row> {
    rows.iter().filter(|r| r.skip.is_none())
}

/// The triage bucket of a case kiln fails, by class and, for the two
/// classes the oracles split, by clang's result (spec/harness.md "The
/// g++.dg runner"): `None` when kiln passes or did not run.
pub fn bucket(row: &Row) -> Option<&'static str> {
    let k = row.class(Role::Kiln)?;
    let clang = row.class(Role::Clang);
    Some(match k {
        Class::Pass | Class::NotRun | Class::ExpectUnknown => return None,
        Class::Crash | Class::Timeout => "kiln crash / hang",
        Class::Wrong => "wrong-code",
        Class::CompileReject => match clang {
            Some(Class::Pass) => "rejects-valid, clang accepts",
            Some(_) => "rejects-valid, clang rejects too",
            None => "rejects-valid, no clang result",
        },
        Class::AcceptInvalid => match clang {
            Some(Class::AcceptInvalid) => "accepts-invalid, clang accepts too",
            Some(_) => "accepts-invalid, clang rejects",
            None => "accepts-invalid, no clang result",
        },
        Class::WrongLine => match clang {
            Some(Class::Pass) => "wrong-line, clang matches",
            Some(_) => "wrong-line, clang differs too",
            None => "wrong-line, no clang result",
        },
    })
}

/// The bucket order of the report.
pub const BUCKETS: [&str; 11] = [
    "kiln crash / hang",
    "wrong-code",
    "rejects-valid, clang accepts",
    "rejects-valid, clang rejects too",
    "rejects-valid, no clang result",
    "accepts-invalid, clang rejects",
    "accepts-invalid, clang accepts too",
    "accepts-invalid, no clang result",
    "wrong-line, clang matches",
    "wrong-line, clang differs too",
    "wrong-line, no clang result",
];

/// The clusters of a bucket, largest first, with up to three examples.
pub fn clusters(rows: &[Row], bucket_name: &str) -> Vec<(String, Vec<String>)> {
    let mut m: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for row in ran_rows(rows) {
        if bucket(row) == Some(bucket_name)
            && let Some(ran) = row.results.get(&Role::Kiln)
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

/// `kiln: 1200/1500 pass; N expect-unknown; N skip rows`: the cases whose
/// expectation the file does not decide are not in the pass count's base.
pub fn summary_line(rows: &[Row]) -> String {
    let run = ran_rows(rows).count();
    let pass = ran_rows(rows).filter(|x| x.class(Role::Kiln) == Some(Class::Pass)).count();
    let unknown = expect_unknown(rows).len();
    format!(
        "kiln: {pass}/{} pass; {unknown} expect-unknown; {} skip rows",
        run - unknown,
        rows.len() - run
    )
}

/// The cases kiln's result leaves undecided by the test file, by the
/// directive that makes them so (the detail before its first `: `),
/// largest first, with their case names.
pub fn expect_unknown(rows: &[Row]) -> Vec<(String, Vec<String>)> {
    let mut m: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for row in ran_rows(rows) {
        if let Some(ran) = row.results.get(&Role::Kiln)
            && ran.verdict.class == Class::ExpectUnknown
        {
            let d = &ran.verdict.detail;
            let why = d.split_once(": ").map_or(d.as_str(), |(w, _)| w);
            let why = why.split(" at L").next().unwrap_or(why);
            m.entry(why.to_owned()).or_default().push(format!("{} {}", row.test, row.opt));
        }
    }
    let mut v: Vec<_> = m.into_iter().collect();
    v.sort_by(|a, b| b.1.len().cmp(&a.1.len()).then_with(|| a.0.cmp(&b.0)));
    v
}

pub const BEGIN: &str = "<!-- gxx-dg:begin (generated by cargo xtask gxx-dg) -->";
pub const END: &str = "<!-- gxx-dg:end -->";

/// The directories reported first: GCC 16.1 is kiln's reference compiler
/// for them.
pub const PRIORITY: [&str; 7] = [
    "g++.dg/reflect",
    "g++.dg/contracts",
    "g++.dg/cpp26",
    "g++.dg/cpp2a",
    "g++.dg/cpp23",
    "g++.dg/torture",
    "c-c++-common/torture",
];

fn md(s: &str) -> String {
    s.replace('|', "\\|")
}

const COLS: [Class; 8] = [
    Class::Pass,
    Class::CompileReject,
    Class::AcceptInvalid,
    Class::WrongLine,
    Class::Wrong,
    Class::Crash,
    Class::Timeout,
    Class::ExpectUnknown,
];

fn class_row(o: &mut String, name: &str, sel: &[&Row]) {
    let counts: Vec<String> = COLS
        .iter()
        .map(|&c| sel.iter().filter(|r| r.class(Role::Kiln) == Some(c)).count().to_string())
        .collect();
    let _ = writeln!(o, "| {} | {} | {} |", md(name), sel.len(), counts.join(" | "));
}

fn class_header(o: &mut String, first: &str) {
    let labels: Vec<&str> = COLS.iter().map(|c| c.label()).collect();
    let _ = writeln!(o, "| {first} | cases | {} |", labels.join(" | "));
    let _ = writeln!(o, "|---|---:|{}", "---:|".repeat(COLS.len()));
}

/// The summary: the region `--report` splices into a status page.
#[allow(clippy::too_many_lines, reason = "one straight-line document")]
pub fn render(info: &RunInfo, rows: &[Row], roles: &[Role]) -> String {
    let mut o = String::new();
    let _ = writeln!(o, "{BEGIN}\n");
    let _ = writeln!(
        o,
        "GCC C++ testsuite (`g++.dg`, `c-c++-common`, `g++.old-deja`, `{GCC_TAG}`), host target `{}`.\n",
        info.triplet
    );
    let _ = writeln!(o, "- tests: GCC `{}`; kiln `{}`", info.tests_commit, info.kiln_commit);
    let _ = writeln!(o, "- command: `{}`", info.command);
    for (r, v) in &info.compilers {
        let _ = writeln!(o, "- {}: {}", r.name(), md(v));
    }
    let _ = writeln!(
        o,
        "- oracles: {}",
        if info.oracles_always {
            "every case"
        } else {
            "only the cases kiln does not pass (their counts below cover those cases)"
        }
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
    let ran_by: Vec<String> = roles
        .iter()
        .map(|&r| ran_rows(rows).filter(|x| x.class(r).is_some()).count().to_string())
        .collect();
    let _ = writeln!(o, "| total | {} |\n", ran_by.join(" | "));

    let _ = writeln!(o, "### kiln's failures by triage bucket (cases)\n");
    let _ = writeln!(o, "| bucket | cases | GCC's testsuite marks xfail |");
    let _ = writeln!(o, "|---|---:|---:|");
    for b in BUCKETS {
        let sel: Vec<&Row> = ran_rows(rows).filter(|r| bucket(r) == Some(b)).collect();
        if !sel.is_empty() {
            let x = sel.iter().filter(|r| r.xfail).count();
            let _ = writeln!(o, "| {b} | {} | {x} |", sel.len());
        }
    }
    o.push('\n');

    let _ = writeln!(o, "### Priority directories (kiln, cases)\n");
    class_header(&mut o, "directory");
    for p in PRIORITY {
        let sel: Vec<&Row> = ran_rows(rows).filter(|r| r.group == p).collect();
        class_row(&mut o, p, &sel);
    }
    o.push('\n');

    let _ = writeln!(o, "### By directory (kiln, cases)\n");
    class_header(&mut o, "directory");
    let groups: BTreeSet<&str> = rows.iter().map(|r| r.group.as_str()).collect();
    for g in &groups {
        let sel: Vec<&Row> = ran_rows(rows).filter(|r| r.group == *g).collect();
        if !sel.is_empty() {
            class_row(&mut o, g, &sel);
        }
    }
    let all: Vec<&Row> = ran_rows(rows).collect();
    class_row(&mut o, "total", &all);
    o.push('\n');

    for b in BUCKETS {
        let cl = clusters(rows, b);
        if cl.is_empty() {
            continue;
        }
        let _ = writeln!(o, "### Clusters: {b} ({} clusters, top 30)\n", cl.len());
        let _ = writeln!(o, "| cases | cluster | examples |");
        let _ = writeln!(o, "|---:|---|---|");
        for (k, v) in cl.iter().take(30) {
            let ex: Vec<&str> = v.iter().take(3).map(String::as_str).collect();
            let _ = writeln!(o, "| {} | {} | {} |", v.len(), md(k), md(&ex.join(", ")));
        }
        o.push('\n');
    }

    let unknown = expect_unknown(rows);
    if !unknown.is_empty() {
        let n: usize = unknown.iter().map(|(_, v)| v.len()).sum();
        let _ = writeln!(
            o,
            "### Expectation unknown ({n} cases, neither pass nor failure)\n\n\
             kiln reported an error the test does not require, where the test file \
             does not decide whether GCC's output holds one (spec/harness.md \"The \
             g++.dg runner\"). By the directive that leaves it open:\n"
        );
        let _ = writeln!(o, "| cases | directive | examples |");
        let _ = writeln!(o, "|---:|---|---|");
        for (k, v) in unknown.iter().take(30) {
            let ex: Vec<&str> = v.iter().take(3).map(String::as_str).collect();
            let _ = writeln!(o, "| {} | `{}` | {} |", v.len(), md(k), md(&ex.join(", ")));
        }
        o.push('\n');
    }

    let _ = writeln!(o, "### Not applicable\n");
    let sk = ranked_skips(rows);
    if !sk.is_empty() {
        let _ = writeln!(o, "Skipped cases (rows; `*` rows cover every case of the test):\n");
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
    if !info.unchecked.is_empty() {
        let mut v: Vec<(&String, &usize)> = info.unchecked.iter().collect();
        v.sort_by(|a, b| b.1.cmp(a.1).then_with(|| a.0.cmp(b.0)));
        let _ = writeln!(
            o,
            "GCC-internal checks of the tests run, never compared (tests; the case's class comes from its compile and run):\n"
        );
        let _ = writeln!(o, "| tests | check |");
        let _ = writeln!(o, "|---:|---|");
        for (k, n) in v.iter().take(25) {
            let _ = writeln!(o, "| {n} | `{}` |", md(k));
        }
        o.push('\n');
    }
    if !info.own.is_empty() {
        let _ = writeln!(o, "Directories their own `.exp` drives (not run; files):\n");
        let parts: Vec<String> = info.own.iter().map(|(d, n)| format!("`{d}` {n}")).collect();
        let _ = writeln!(o, "{}\n", parts.join(", "));
    }

    let dropped: Vec<String> = info
        .dropped
        .iter()
        .filter(|(_, d)| !d.is_empty())
        .map(|(r, d)| format!("- {}: {}", r.name(), dropped_flags(d)))
        .collect();
    if !dropped.is_empty() {
        let _ = writeln!(o, "### Flags a compiler rejected (dropped from its cases)\n");
        for d in dropped {
            let _ = writeln!(o, "{d}");
        }
        o.push('\n');
    }
    let _ = writeln!(o, "{END}");
    o
}

/// `w` with an absolute path (the whole word, or after `=`) as `<path>`.
fn path_masked(w: &str) -> String {
    match w.split_once("=/") {
        _ if w.starts_with('/') => "<path>".to_owned(),
        Some((k, _)) => format!("{k}=<path>"),
        None => w.to_owned(),
    }
}

/// How many distinct dropped flags the report lists per compiler.
const DROPPED_SHOWN: usize = 60;

/// A compiler's dropped flag units for the report: without the standard
/// they were probed under, absolute paths as `<path>`, each flag once, the
/// first [`DROPPED_SHOWN`] in order and a count of the rest.
pub fn dropped_flags(units: &[String]) -> String {
    let flags: BTreeSet<String> = units
        .iter()
        .map(|u| {
            let words: Vec<&str> = u.split(' ').filter(|w| !w.starts_with("-std=")).collect();
            let words: Vec<String> = words.iter().map(|w| path_masked(w)).collect();
            words.join(" ")
        })
        .filter(|f| !f.is_empty())
        .collect();
    let shown: Vec<String> = flags.iter().take(DROPPED_SHOWN).map(|f| format!("`{f}`")).collect();
    let mut out = shown.join(", ");
    if flags.len() > DROPPED_SHOWN {
        let _ = write!(out, " and {} more", flags.len() - DROPPED_SHOWN);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_limit_lifted() {
        // GCC has no error limit; kiln and clang stop after 20 by default.
        assert_eq!(unlimited_errors(Role::Kiln), Some("-ferror-limit=0"));
        assert_eq!(unlimited_errors(Role::Clang), Some("-ferror-limit=0"));
        assert_eq!(unlimited_errors(Role::Gcc), None);
    }

    #[test]
    fn dropped_flag_list() {
        let units: Vec<String> = [
            "-std=c++11 -fchecking",
            "-std=c++20 -fchecking",
            "-std=c++20 -include /a/b.h",
            "-std=c++98 /a/c.C",
            "-std=c++98 --x=/a",
        ]
        .iter()
        .map(|s| (*s).to_owned())
        .collect();
        assert_eq!(
            dropped_flags(&units),
            "`--x=<path>`, `-fchecking`, `-include <path>`, `<path>`"
        );
        let many: Vec<String> = (0..70).map(|i| format!("-std=c++20 -fx{i:02}")).collect();
        assert!(dropped_flags(&many).ends_with("`-fx59` and 10 more"), "{}", dropped_flags(&many));
    }

    fn args(a: &[&str]) -> Vec<String> {
        a.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn parses_options() {
        let o = parse_args(args(&[
            "--suite",
            "dg,torture",
            "--std=20,26",
            "--oracles-on",
            "all",
            "--sample",
            "500",
            "-j",
            "3",
        ]))
        .unwrap()
        .unwrap();
        assert_eq!(o.suites, [Suite::Dg, Suite::Torture]);
        assert_eq!(o.stds, Some(args(&["20", "26"])));
        assert!(o.oracles_always);
        assert_eq!((o.sample, o.jobs), (Some(500), 3));
        assert_eq!(parse_args(args(&["--std", "gcc"])).unwrap().unwrap().stds, None);
        assert!(parse_args(args(&["--std", "27"])).is_err());
        assert!(parse_args(args(&["--suite", "gcc.dg"])).is_err());
        assert!(parse_args(args(&["--oracles-on", "some"])).is_err());
        assert!(parse_args(args(&["--help"])).unwrap().is_none());
    }

    /// `--shard K/N` (spec/harness.md "The g++.dg runner"): the N shards
    /// of a selection are disjoint, cover it and keep its order, and a
    /// test's cases never split (shards are taken over tests, not cases).
    #[test]
    fn shards_partition_the_tests() {
        assert_eq!(parse_args(args(&["--shard", "2/4"])).unwrap().unwrap().shard, Some((2, 4)));
        assert_eq!(parse_args(args(&["--shard=1/1"])).unwrap().unwrap().shard, Some((1, 1)));
        for bad in ["0/3", "4/3", "3", "a/b", "1/0"] {
            assert!(parse_args(args(&["--shard", bad])).is_err(), "{bad:?}");
        }
        let all: Vec<Test> =
            (0..23).map(|i| test(&format!("g++.dg/t{i:02}.C"), Suite::Dg)).collect();
        let n = 4;
        let parts: Vec<Vec<Test>> =
            (1..=n).map(|k| crate::libcxx::shard_of(all.clone(), (k, n))).collect();
        let mut seen: Vec<String> = parts.iter().flatten().map(|t| t.id.clone()).collect();
        assert_eq!(seen.len(), all.len(), "the shards overlap or drop tests");
        seen.sort();
        let want: Vec<String> = all.iter().map(|t| t.id.clone()).collect();
        assert_eq!(seen, want);
        for p in &parts {
            assert!(p.windows(2).all(|w| w[0].id < w[1].id), "a shard reorders its tests");
            assert!(p.len().abs_diff(all.len() / n) <= 1, "unbalanced shard: {}", p.len());
        }
        let o = Options { shard: Some((3, 4)), ..Options::default() };
        assert!(command_line(&o).ends_with(" --shard 3/4"), "{}", command_line(&o));
    }

    #[test]
    fn standards_follow_gxx_dg_exp() {
        let none: &[String] = &[];
        // A plain test: cxx_default first, then the oldest and newest.
        assert_eq!(
            std_flags("int a;\n", "g++.dg/a.C", none),
            args(&["-std=c++20", "-std=c++98", "-std=c++26"])
        );
        // dg-options allows GNU extensions.
        assert_eq!(
            std_flags("// { dg-options \"-O2\" }\n", "g++.dg/a.C", none),
            args(&["-std=gnu++20", "-std=gnu++98", "-std=gnu++26"])
        );
        // A named version on a dg-do line is the floor and is tested.
        assert_eq!(
            std_flags("// { dg-do compile { target c++17 } }\n", "g++.dg/a.C", none),
            args(&["-std=c++17", "-std=c++20", "-std=c++26"])
        );
        // Named on a dg-error line: tested, but no floor.
        assert_eq!(
            std_flags("int x; // { dg-error \"\" \"\" { target c++23 } }\n", "g++.dg/a.C", none),
            args(&["-std=c++23", "-std=c++20", "-std=c++98"])
        );
        // c++98 never raises the floor.
        assert_eq!(
            std_flags("// { dg-do run { target c++98_only } }\n", "g++.dg/a.C", none),
            args(&["-std=c++98", "-std=c++20", "-std=c++26"])
        );
        // The test's own -std=: one run with nothing added.
        assert_eq!(std_flags("// { dg-options \"-std=c++17\" }\n", "g++.dg/a.C", none), [""]);
        // Directory floors.
        assert_eq!(
            std_flags("int a;\n", "g++.dg/coroutines/a.C", none),
            args(&["-std=c++20", "-std=c++26", "-std=c++23"])
        );
        // GXX_TESTSUITE_STDS, filtered by the floor.
        let l = args(&["98", "17", "26"]);
        assert_eq!(
            std_flags("int a;\n", "g++.dg/a.C", &l),
            args(&["-std=c++98", "-std=c++17", "-std=c++26"])
        );
        assert_eq!(std_flags("int a;\n", "g++.dg/coroutines/a.C", &l), args(&["-std=c++26"]));
    }

    fn test(id: &str, suite: Suite) -> Test {
        Test { suite, id: id.into(), path: PathBuf::from(format!("/ts/{id}")) }
    }

    #[test]
    fn plans_cases() {
        let t = Target::for_arch("x86_64", false);
        let body = "// { dg-do compile { target c++20 } }\n\
                    int x = y; // { dg-error \"undeclared\" }\n\
                    // { dg-final { scan-assembler \"foo\" } }\n";
        let p = plan_test(&test("g++.dg/cpp2a/a.C", Suite::Dg), body, &[], &[], &t);
        let labels: Vec<&str> = p.iter().map(|(l, _)| l.as_str()).collect();
        assert_eq!(labels, ["-std=c++20", "-std=c++26", "-std=c++23"]);
        let Ok(c) = &p[0].1 else { panic!("{p:?}") };
        assert_eq!(c.flags, args(&["-std=c++20", "-pedantic-errors", "-Wno-long-long"]));
        assert!(c.plan.negative && c.expected.negative());
        assert_eq!(unchecked(&dg::directives(body)), ["scan-assembler"]);
        // A selector on the standard skips at the others.
        let body = "// { dg-do run { target c++17_only } }\n";
        let p = plan_test(&test("g++.dg/a.C", Suite::Dg), body, &[], &[], &t);
        assert!(p.iter().filter(|(_, c)| c.is_ok()).count() == 1, "{p:?}");
        // Torture: the option sets, GCC's default standard, no default flags.
        let p = plan_test(
            &test("g++.dg/torture/a.C", Suite::Torture),
            "// { dg-do run }\n",
            &[],
            &args(&["-O0", "-O2"]),
            &t,
        );
        let Ok(c) = &p[1].1 else { panic!() };
        assert_eq!(c.flags, args(&[CXX_DEFAULT, "-O2"]));
        assert_eq!(c.plan.action, Action::Run);
        // The test's own -std= decides the standard selectors see.
        let body =
            "// { dg-options \"-std=c++14\" }\nint a; // { dg-error \"\" \"\" { target c++17 } }\n";
        let p = plan_test(&test("g++.dg/a.C", Suite::Dg), body, &[], &[], &t);
        assert_eq!(p.len(), 1);
        let Ok(c) = &p[0].1 else { panic!() };
        assert!(!c.expected.negative());
        assert_eq!(c.flags, args(&["-std=c++14"]));
        // Modules and assembler input are counted, not run.
        let p = plan_test(&test("g++.dg/m.C", Suite::Dg), "// { dg-module-cmi M }\n", &[], &[], &t);
        assert_eq!(p, vec![("*".to_owned(), Err("modules (dg-module-*)".to_owned()))]);
        let p = plan_test(&test("c-c++-common/a.S", Suite::Common), "", &[], &[], &t);
        assert!(matches!(&p[0].1, Err(w) if w.starts_with("input kind .S")));
        // dg-additional-sources sit beside the test; dg-shouldfail is kept.
        let p = plan_test(
            &test("g++.dg/a.C", Suite::Dg),
            "// { dg-do run }\n// { dg-additional-sources \"b.cc\" }\n// { dg-shouldfail \"x\" }\n",
            &args(&["26"]),
            &[],
            &t,
        );
        let Ok(c) = &p[0].1 else { panic!() };
        assert_eq!(c.sources, [PathBuf::from("/ts/g++.dg/a.C"), PathBuf::from("/ts/g++.dg/b.cc")]);
        assert!(c.shouldfail);
        let p = plan_test(
            &test("c-c++-common/cpp/e.c", Suite::Common),
            "/* { dg-options \"--embed-directory ${srcdir}/c-c++-common/cpp/d\" } */\n",
            &args(&["26"]),
            &[],
            &t,
        );
        let Ok(c) = &p[0].1 else { panic!() };
        assert_eq!(c.flags, args(&["-std=gnu++26", "--embed-directory", "/ts/c-c++-common/cpp/d"]));
        assert_eq!(should_fail(Verdict::new(Class::Wrong, "exit 1")), Verdict::pass());
        assert_eq!(should_fail(Verdict::pass()).class, Class::Wrong);
    }

    /// Several sources and options with paths (WP-1664): input files in
    /// the options join the sources, `$srcdir` and `$subdir` are
    /// substituted, `dg-additional-sources` takes selectors and `linkonly`.
    #[test]
    fn sources_and_substitutions() {
        let t = Target::for_arch("x86_64", false);
        let ok = |id: &str, suite: Suite, body: &str| {
            let p = plan_test(&test(id, suite), body, &args(&["26"]), &[], &t);
            match p.into_iter().next() {
                Some((_, Ok(c))) => c,
                other => panic!("{other:?}"),
            }
        };
        let c = ok(
            "g++.dg/ipa/a-2.C",
            Suite::Dg,
            "/* { dg-do run } */\n/* { dg-options \"-O3 ${srcdir}/g++.dg/ipa/a-1.C\" } */\n",
        );
        assert_eq!(c.flags, args(&["-std=gnu++26", "-O3"]));
        assert_eq!(
            c.sources,
            [PathBuf::from("/ts/g++.dg/ipa/a-2.C"), PathBuf::from("/ts/g++.dg/ipa/a-1.C")]
        );
        // A path that is the value of a flag stays a flag.
        let c = ok(
            "c-c++-common/cpp/i.c",
            Suite::Common,
            "// { dg-options \"-include $srcdir/c-c++-common/cpp/i.h -I$subdir/x\" }\n\
             // { dg-additional-options \"-imacros ${srcdir}/m.c\" }\n",
        );
        assert_eq!(
            c.flags,
            args(&[
                "-std=gnu++26",
                "-include",
                "/ts/c-c++-common/cpp/i.h",
                "-Ig++.dg/x",
                "-imacros",
                "/ts/m.c"
            ])
        );
        assert_eq!(c.sources.len(), 1);
        assert_eq!(with_srcdir("${subdir}/a", "/s", subdir_of(Suite::OldDeja)), "g++.old-deja/a");
        assert_eq!(with_srcdir("$subdir", "/s", subdir_of(Suite::Torture)), "g++.dg/torture");
        // Several words, several directives, a selector, linkonly.
        let body = "// { dg-do link }\n\
                    // { dg-additional-sources \"b.cc c.cc\" }\n\
                    // { dg-additional-sources \"d.cc\" { target c++20 } }\n\
                    // { dg-additional-sources \"e.cc\" { target c++11_only } }\n\
                    // { dg-additional-sources \"f.cc\" linkonly }\n";
        let c = ok("g++.dg/x/a.C", Suite::Dg, body);
        let names: Vec<_> =
            c.sources.iter().map(|p| p.file_name().unwrap().to_str().unwrap()).collect();
        assert_eq!(names, ["a.C", "b.cc", "c.cc", "d.cc", "f.cc"]);
        let c = ok("g++.dg/x/a.C", Suite::Dg, &body.replace("dg-do link", "dg-do compile"));
        assert_eq!(c.sources.len(), 4, "linkonly: only when the case links");
        let p = plan_test(
            &test("g++.dg/a.C", Suite::Dg),
            "// { dg-additional-sources \"b.cc\" { xfail *-*-* } }\n",
            &args(&["26"]),
            &[],
            &t,
        );
        assert!(matches!(&p[0].1, Err(e) if e.starts_with("dg-additional-sources")), "{p:?}");
        assert_eq!(input_file(&args(&["x.o"])), Some("x.o"));
        assert_eq!(input_file(&args(&["-include", "x.c"])), None);
        assert_eq!(input_file(&args(&["-fx.c"])), None);
        assert_eq!(input_file(&args(&["dir"])), None);
    }

    /// A missing source is a skip, not a failure (WP-1664).
    #[test]
    fn missing_sources_skip() {
        let d = std::env::temp_dir().join(format!("xtask-gxx-missing-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let a = d.join("a.C");
        std::fs::write(&a, "// { dg-do run }\n// { dg-additional-sources \"gone.cc\" }\n").unwrap();
        let tests = [Test { suite: Suite::Dg, id: "g++.dg/a.C".into(), path: a }];
        let bodies = [std::fs::read_to_string(&tests[0].path).unwrap()];
        let t = Target::for_arch("x86_64", false);
        let (jobs, skips, _) = plan_all(&tests, &bodies, &args(&["26"]), &[], &t);
        let _ = std::fs::remove_dir_all(&d);
        assert!(jobs.is_empty());
        assert_eq!(skips[0].skip.as_deref(), Some("missing source gone.cc"));
    }

    /// Two sources compiled and linked, run; an expected link error
    /// (WP-1664), through a real compiler.
    #[test]
    fn links_several_sources_and_expects_link_errors() {
        let cxx = PathBuf::from("clang++-19");
        if std::process::Command::new(&cxx).arg("--version").output().is_err() {
            eprintln!("skipping: clang++-19 not available");
            return;
        }
        let dir = std::env::temp_dir().join(format!("xtask-gxx-link-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let write = |n: &str, s: &str| std::fs::write(dir.join(n), s).unwrap();
        write("def.cc", "int helper() { return 7; }\n");
        write(
            "run.C",
            "// { dg-do run }\n// { dg-additional-sources \"def.cc\" }\n\
             int helper();\nint main() { return helper() == 7 ? 0 : 1; }\n",
        );
        write("undef.C", "// { dg-do link }\nint helper();\nint main() { return helper(); }\n");
        write(
            "linkerr.C",
            "// { dg-do link }\n// { dg-message \"helper\" \"\" { target *-*-* } 0 }\n\
             int helper();\nint main() { return helper(); }\n",
        );
        write(
            "linked.C",
            "// { dg-do link }\n// { dg-additional-sources \"def.cc\" }\n\
             // { dg-error \"helper\" \"\" { target *-*-* } 0 }\n\
             int helper();\nint main() { return helper(); }\n",
        );
        let tool = Tool::new_for(Role::Clang, cxx, Vec::new(), &dir, "probe.cc").unwrap();
        let t = Target::host();
        let class = |name: &str| {
            let test =
                Test { suite: Suite::Dg, id: format!("g++.dg/{name}"), path: dir.join(name) };
            let body = std::fs::read_to_string(&test.path).unwrap();
            let p = plan_test(&test, &body, &args(&["17"]), &[], &t);
            let Some((_, Ok(c))) = p.into_iter().next() else { panic!("{name}") };
            let link = args(&["-lm"]);
            let case = Case {
                sources: &c.sources,
                action: c.plan.action,
                flags: &c.flags,
                link_flags: &link,
                negative: c.plan.negative,
                compile_timeout: Duration::from_secs(60),
                run_timeout: Duration::from_secs(10),
                mem: None,
                lang: None,
                expect: Some(&c.expected),
            };
            let v = tool.run_case(&case, &dir.join(format!("w-{name}"))).verdict;
            (v.class, v.detail)
        };
        let got = [class("run.C"), class("undef.C"), class("linkerr.C"), class("linked.C")];
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(got[0].0, Class::Pass, "{:?}", got[0]);
        assert_eq!(got[1].0, Class::CompileReject, "{:?}", got[1]);
        assert!(got[1].1.starts_with("link:"), "{:?}", got[1]);
        assert_eq!(got[2].0, Class::Pass, "the expected link error: {:?}", got[2]);
        assert_eq!(
            got[3],
            (
                Class::AcceptInvalid,
                "linked (expected a link error: dg-error \"helper\")".to_owned()
            )
        );
    }

    #[test]
    fn groups_and_samples() {
        assert_eq!(group_of("g++.dg/cpp26/a.C"), "g++.dg/cpp26");
        assert_eq!(group_of("g++.dg/a.C"), "g++.dg");
        assert_eq!(group_of("g++.dg/contracts/cpp26/a.C"), "g++.dg/contracts");
        assert_eq!(group_of("c-c++-common/torture/a.c"), "c-c++-common/torture");
        let v: Vec<usize> = (0..10).collect();
        assert_eq!(sample(&v, 3), [0, 3, 6]);
        assert_eq!(sample(&v, 20), v);
    }

    fn row(test: &str, k: Class, detail: &str, clang: Option<Class>) -> Row {
        let ran = |c: Class, d: &str| Ran { verdict: Verdict::new(c, d), dropped: Vec::new() };
        let mut results = BTreeMap::from([(Role::Kiln, ran(k, detail))]);
        if let Some(c) = clang {
            results.insert(Role::Clang, ran(c, ""));
        }
        Row {
            test: test.into(),
            group: group_of(test),
            opt: "-std=c++20".into(),
            skip: None,
            xfail: false,
            negative: false,
            results,
        }
    }

    #[test]
    fn buckets_and_report() {
        let rows = vec![
            row("g++.dg/cpp26/a.C", Class::CompileReject, "error: no 'x'", Some(Class::Pass)),
            row("g++.dg/cpp26/b.C", Class::CompileReject, "error: no 'y'", Some(Class::Pass)),
            row("g++.dg/c.C", Class::CompileReject, "error: ext", Some(Class::CompileReject)),
            row("g++.dg/d.C", Class::AcceptInvalid, "no error", Some(Class::Pass)),
            row("g++.dg/e.C", Class::AcceptInvalid, "no error", Some(Class::AcceptInvalid)),
            row("g++.dg/f.C", Class::Crash, "panicked", None),
            row("g++.dg/g.C", Class::Pass, "", None),
        ];
        assert_eq!(bucket(&rows[0]), Some("rejects-valid, clang accepts"));
        assert_eq!(bucket(&rows[2]), Some("rejects-valid, clang rejects too"));
        assert_eq!(bucket(&rows[3]), Some("accepts-invalid, clang rejects"));
        assert_eq!(bucket(&rows[4]), Some("accepts-invalid, clang accepts too"));
        assert_eq!(bucket(&rows[5]), Some("kiln crash / hang"));
        assert_eq!(bucket(&rows[6]), None);
        let cl = clusters(&rows, "rejects-valid, clang accepts");
        assert_eq!(cl.len(), 1);
        assert_eq!(cl[0].1.len(), 2);
        let region = render(&RunInfo::default(), &rows, &[Role::Kiln, Role::Clang]);
        assert!(region.starts_with(BEGIN) && region.trim_end().ends_with(END));
        assert!(region.contains("| rejects-valid, clang accepts | 2 | 0 |"), "{region}");
        assert!(region.contains("| g++.dg/cpp26 | 2 | 0 | 2 |"), "{region}");
        assert_eq!(summary_line(&rows), "kiln: 1/7 pass; 0 expect-unknown; 0 skip rows");
        assert_eq!(tsv(&rows).lines().count(), 8);
    }
}
