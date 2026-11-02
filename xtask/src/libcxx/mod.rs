//! `cargo xtask libcxx` — runs libc++'s own test suite (`libcxx/test` of a
//! pinned LLVM release) with kiln as the compiler. No lit and no `CMake`: the
//! classification, keywords and feature evaluation of libc++'s `format.py`
//! are reimplemented for the subset the suite needs. Contract:
//! `spec/libcxx.md`.

mod boolexpr;
pub(crate) mod cluster;
mod directives;
mod features;
mod report;
mod run;

use std::collections::HashMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::util::process::{self, MemLimit, RssProbe};
use crate::util::{Profile, build_kiln_with, repo_root, target_dir};
use directives::{Kind, LocalCfg};
use run::{Config, Job, LibcxxTree, Outcome, TestResult};

const USAGE: &str = "\
usage: cargo xtask libcxx [options] [<dir>...]

Runs libc++'s test suite (llvmorg-19.1.7 `libcxx/test`) with kiln. <dir>s are
relative to libcxx/test (default: --gate m3). Contract: spec/libcxx.md.

  --gate m3|m4           a gate's directories (spec/libcxx.md \"Gates\"); m3 runs
                         llvmorg-19.1.7, m4 llvmorg-22.1.8 unless --tag says otherwise
  --tag <llvmorg-X.Y.Z>  the release whose checkout to use (default: the gate's,
                         else $KILN_LIBCXX_TAG, else llvmorg-19.1.7); its major
                         version picks the clang/libc++ release kiln++ runs with
  --libcxx-tests <dir>   the llvm-project checkout (default: .toolchain/libcxx-<tag>,
                         `scripts/fetch-libcxx-tests.sh <tag>`; for llvmorg-19.1.7
                         also the older .toolchain/llvm-project)
  --profile <p>          the cargo profile to build kiln++ with: debug, harness
                         (default) or release (spec/libcxx.md \"Build profiles\")
  --compiler <path>      kiln++ binary (default: cargo build --profile <p> -p kiln_driver)
  --filter <substring>   only tests whose path contains <substring>
  --list                 list the selected tests and their pre-run verdict
  -j, --jobs <n>         tests in parallel (default 2; an upper bound under a
                         MAKEFLAGS jobserver, see spec/harness.md)
  --timeout <secs>       test binary timeout (default 30)
  --run-timeout-long <secs>
                         test binary timeout of a long test: REQUIRES: long_tests,
                         or listed in xtask/src/libcxx/slow_tests.txt (default 240)
  --compile-timeout <secs>
                         compile/link step timeout (default 120; --profile debug: 300)
  --mem-limit <MiB>      address-space cap per process (default 8192; 0: none)
  --report <file.md>     write the status page's generated region to <file.md>
  --keep                 keep the scratch directories
  --no-fail              exit 0 even on FAIL, XPASS, KILLED, TIMEOUT or ERROR
  -v, --verbose          print every result and failure details

environment:
  KILN_LIBCXX_TAG        the default --tag
  KILN_ORACLE            only for a tag whose major is not 19 or 22: kiln++ runs with
                         KILN_LLVM_DIR=/usr/lib/llvm-N of the tag's major (19 or 22),
                         else of KILN_ORACLE (spec/libcxx.md \"Toolchain\")
";

/// The directories of the M3 gate, relative to `libcxx/test`.
pub const GATE_M3: &[&str] = &[
    "std/utilities/meta",
    "std/utilities/utility",
    "std/containers/sequences/array",
    "std/strings/string.view",
    "std/containers/views/views.span",
    "std/numerics/bit",
    "std/language.support/support.limits",
    "std/language.support/support.initlist",
];

/// The directories of the M4 libc++ exit (docs/ROADMAP.md M4: `<vector>`,
/// `<string>`, `<memory>`, `<optional>`, `<variant>`, `<tuple>`,
/// `<algorithm>`, `<ranges>`, `<format>`, `<expected>`, `<mdspan>`,
/// `<print>`, `<flat_map>`), run after [`GATE_M3`]'s.
pub const GATE_M4_EXTRA: &[&str] = &[
    "std/containers/sequences/vector",
    "std/strings/basic.string",
    "std/utilities/memory",
    "std/utilities/optional",
    "std/utilities/variant",
    "std/utilities/tuple",
    "std/algorithms",
    "std/ranges",
    "std/utilities/format",
    "std/utilities/expected",
    "std/containers/views/mdspan",
    "std/input.output/iostream.format/print.fun",
    "std/containers/container.adaptors/flat.map",
];

/// The release whose tests the driver targets by default (and the M3 gate's).
pub const TESTS_TAG: &str = "llvmorg-19.1.7";

/// The release of the M4 gate's tests.
pub const M4_TAG: &str = "llvmorg-22.1.8";

/// The gate's directories, relative to `libcxx/test`.
fn gate_dirs(gate: &str) -> Vec<String> {
    let mut v: Vec<String> = GATE_M3.iter().map(|s| (*s).to_owned()).collect();
    if gate == "m4" {
        v.extend(GATE_M4_EXTRA.iter().map(|s| (*s).to_owned()));
    }
    v
}

/// The tests release of a gate.
fn gate_tag(gate: &str) -> &'static str {
    if gate == "m4" { M4_TAG } else { TESTS_TAG }
}

/// The environment variable naming the default `--tag`.
pub const TAG_ENV: &str = "KILN_LIBCXX_TAG";

/// Checks an llvm-project release tag (`llvmorg-<version>`), which also
/// names a directory (`.toolchain/libcxx-<tag>`).
fn check_tag(tag: &str) -> Result<(), String> {
    let ok = tag.strip_prefix("llvmorg-").is_some_and(|v| {
        !v.is_empty() && v.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
    });
    if ok { Ok(()) } else { Err(format!("bad libc++ tests tag `{tag}` (want llvmorg-<version>)")) }
}

/// The tag to use: `--tag`, else the `--gate`'s, else `$KILN_LIBCXX_TAG`,
/// else [`TESTS_TAG`].
fn resolve_tag(
    flag: Option<&str>,
    gate: Option<&str>,
    env: Option<&str>,
) -> Result<String, String> {
    let tag = flag.or(gate.map(gate_tag)).or(env.filter(|v| !v.is_empty())).unwrap_or(TESTS_TAG);
    check_tag(tag)?;
    Ok(tag.to_owned())
}

#[derive(Clone, Debug, PartialEq)]
#[allow(clippy::struct_excessive_bools)] // independent command-line switches
pub struct Options {
    pub tests: Option<PathBuf>,
    /// `--tag`; `None` is the gate's, else `$KILN_LIBCXX_TAG`, else [`TESTS_TAG`].
    pub tag: Option<String>,
    pub compiler: Option<PathBuf>,
    /// `--profile`: what the implicit build makes (ignored with `--compiler`
    /// except in the report, which then says the profile is unknown).
    pub profile: Profile,
    pub gate: Option<String>,
    pub dirs: Vec<String>,
    pub filter: Option<String>,
    pub list: bool,
    pub jobs: usize,
    pub run_timeout: Duration,
    /// `--run-timeout-long`: the run limit of a long test.
    pub long_run_timeout: Duration,
    /// `--compile-timeout`; `None` is [`Profile::compile_timeout`].
    pub compile_timeout: Option<Duration>,
    pub mem_limit_mib: u64,
    pub report: Option<PathBuf>,
    pub keep: bool,
    pub no_fail: bool,
    pub verbose: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            tests: None,
            tag: None,
            compiler: None,
            profile: Profile::Harness,
            gate: None,
            dirs: Vec::new(),
            filter: None,
            list: false,
            jobs: 2,
            run_timeout: Duration::from_secs(30),
            long_run_timeout: Duration::from_secs(240),
            compile_timeout: None,
            mem_limit_mib: process::DEFAULT_MEM_LIMIT_MIB,
            report: None,
            keep: false,
            no_fail: false,
            verbose: false,
        }
    }
}

impl Options {
    /// The compile/link step limit: `--compile-timeout`, else the profile's.
    pub fn compile_timeout(&self) -> Duration {
        self.compile_timeout.unwrap_or_else(|| self.profile.compile_timeout())
    }
}

/// A positive, finite number of seconds.
fn parse_secs(flag: &str, v: &str) -> Result<Duration, String> {
    v.parse()
        .ok()
        .filter(|s: &f64| s.is_finite() && *s > 0.0)
        .map(Duration::from_secs_f64)
        .ok_or_else(|| format!("bad {flag} `{v}`"))
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
            "--tag" => {
                let t = value("--tag")?;
                check_tag(&t)?;
                o.tag = Some(t);
            }
            "--libcxx-tests" => o.tests = Some(PathBuf::from(value("--libcxx-tests")?)),
            "--compiler" => o.compiler = Some(PathBuf::from(value("--compiler")?)),
            "--profile" => o.profile = Profile::parse(&value("--profile")?)?,
            "--gate" => {
                let g = value("--gate")?;
                if g != "m3" && g != "m4" {
                    return Err(format!("unknown --gate `{g}` (known: m3, m4)"));
                }
                o.gate = Some(g);
            }
            "--filter" => o.filter = Some(value("--filter")?),
            "--report" => o.report = Some(PathBuf::from(value("--report")?)),
            "-j" | "--jobs" => {
                let v = value("--jobs")?;
                o.jobs =
                    v.parse().ok().filter(|&n| n > 0).ok_or_else(|| format!("bad --jobs `{v}`"))?;
            }
            "--timeout" => o.run_timeout = parse_secs("--timeout", &value("--timeout")?)?,
            "--run-timeout-long" => {
                o.long_run_timeout =
                    parse_secs("--run-timeout-long", &value("--run-timeout-long")?)?;
            }
            "--compile-timeout" => {
                o.compile_timeout =
                    Some(parse_secs("--compile-timeout", &value("--compile-timeout")?)?);
            }
            "--mem-limit" => {
                let v = value("--mem-limit")?;
                o.mem_limit_mib =
                    v.parse().map_err(|_| format!("bad --mem-limit `{v}` (MiB; 0 disables)"))?;
            }
            "--list" | "--keep" | "--no-fail" | "-v" | "--verbose" if inline.is_some() => {
                return Err(format!("`{flag}` takes no value"));
            }
            "--list" => o.list = true,
            "--keep" => o.keep = true,
            "--no-fail" => o.no_fail = true,
            "-v" | "--verbose" => o.verbose = true,
            f if f.starts_with('-') => return Err(format!("unknown option `{f}`")),
            _ => o.dirs.push(arg.trim_end_matches('/').to_owned()),
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
            eprintln!("xtask libcxx: {e}\n\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    match execute(&opts) {
        Ok(failed) if failed && !opts.no_fail => ExitCode::from(1),
        Ok(_) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("xtask libcxx: {e}");
            ExitCode::from(2)
        }
    }
}

/// The default checkout of `tag`: `.toolchain/libcxx-<tag>` of this
/// checkout, else of the main checkout when this is a worktree (where the
/// fetch script puts it); for [`TESTS_TAG`] also the pre-tag location
/// `.toolchain/llvm-project`. The first that exists, else the fetch
/// script's target.
fn default_checkout(root: &Path, tag: &str) -> PathBuf {
    let mut names = vec![format!("libcxx-{tag}")];
    if tag == TESTS_TAG {
        names.push("llvm-project".to_owned());
    }
    let mut bases = vec![root.to_path_buf()];
    if let Some(main) = main_checkout(root) {
        bases.push(main);
    }
    let candidates: Vec<PathBuf> = names
        .iter()
        .flat_map(|n| bases.iter().map(move |b| b.join(".toolchain").join(n)))
        .collect();
    // None yet: where the fetch script puts it (the main checkout's).
    candidates.into_iter().find(|c| c.is_dir()).unwrap_or_else(|| {
        let base = bases.last().map_or(root, PathBuf::as_path);
        base.join(".toolchain").join(format!("libcxx-{tag}"))
    })
}

/// The major version of an `llvmorg-<major>.…` tag.
fn tag_major(tag: &str) -> Option<u32> {
    tag.strip_prefix("llvmorg-")?.split(['.', '-']).next()?.parse().ok()
}

/// Where the libc++ of `toolchain` is: `None` for the system install
/// (`/usr/lib/llvm-N/include/c++/v1`, which `-stdlib=libc++` finds by
/// itself), else an extracted Debian package tree
/// `.toolchain/libcxx-N-root` of this or the main checkout (spec/libcxx.md
/// "Toolchain": `libc++-N-dev` cannot be installed beside `libc++-19-dev`).
fn libcxx_install(
    root: &Path,
    toolchain: crate::oracle::Oracle,
) -> Result<Option<LibcxxTree>, String> {
    if toolchain.libcxx_include().is_dir() {
        return Ok(None);
    }
    let n = toolchain.version;
    let mut bases = vec![root.to_path_buf()];
    if let Some(main) = main_checkout(root) {
        bases.push(main);
    }
    let arch = format!("{}-linux-gnu", std::env::consts::ARCH);
    for b in &bases {
        let tree = b.join(".toolchain").join(format!("libcxx-{n}-root"));
        let include = tree.join(format!("usr/lib/llvm-{n}/include/c++/v1"));
        let lib = tree.join("usr/lib").join(&arch);
        if include.is_dir() && lib.join("libc++.a").is_file() {
            return Ok(Some(LibcxxTree { include, lib }));
        }
    }
    Err(format!(
        "no libc++ {n} headers at {} or in .toolchain/libcxx-{n}-root; install \
         libc++-{n}-dev, or extract libc++-{n}-dev, libc++abi-{n}-dev and libunwind-{n}-dev \
         there (spec/libcxx.md \"Toolchain\")",
        toolchain.libcxx_include().display()
    ))
}

/// The main checkout when `root` is a worktree of it: the common git dir's
/// `core.worktree` (kiln as a submodule), else the common dir's parent.
fn main_checkout(root: &Path) -> Option<PathBuf> {
    let git = |args: &[&str]| {
        Command::new("git")
            .arg("-C")
            .arg(root)
            .args(args)
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
            .filter(|s| !s.is_empty())
    };
    let common = PathBuf::from(git(&["rev-parse", "--path-format=absolute", "--git-common-dir"])?);
    let main = match git(&[
        "config",
        "--file",
        &common.join("config").display().to_string(),
        "core.worktree",
    ]) {
        Some(wt) => common.join(wt),
        None => common.join(".."),
    };
    let main = std::fs::canonicalize(&main).unwrap_or(main);
    main.join("xtask").is_dir().then_some(main)
}

/// All tests under `dirs` (relative to `test_root`), sorted, with `/`
/// separated paths relative to `test_root`. `Inputs` directories are skipped,
/// as lit does.
fn discover(
    test_root: &Path,
    dirs: &[String],
    filter: Option<&str>,
) -> Result<Vec<(PathBuf, String)>, String> {
    let mut out = Vec::new();
    for d in dirs {
        let start = test_root.join(d);
        if !start.exists() {
            return Err(format!("no such test directory: {}", start.display()));
        }
        let mut stack = vec![start];
        while let Some(p) = stack.pop() {
            if p.is_dir() {
                if p.file_name().is_some_and(|n| n == "Inputs") {
                    continue;
                }
                let rd = std::fs::read_dir(&p)
                    .map_err(|e| format!("cannot read {}: {e}", p.display()))?;
                stack.extend(rd.filter_map(|e| e.ok().map(|e| e.path())));
            } else if p.file_name().and_then(|n| n.to_str()).and_then(Kind::of).is_some() {
                let rel = p.strip_prefix(test_root).unwrap_or(&p);
                let rel = rel
                    .components()
                    .map(|c| c.as_os_str().to_string_lossy())
                    .collect::<Vec<_>>()
                    .join("/");
                if filter.is_none_or(|f| rel.contains(f)) && !out.iter().any(|(_, r)| *r == rel) {
                    out.push((p, rel));
                }
            }
        }
    }
    out.sort_by(|a, b| a.1.cmp(&b.1));
    Ok(out)
}

/// The `lit.local.cfg` files from `test_root` down to `dir`, outermost
/// first, parsed and cached.
fn local_cfgs<'a>(
    cache: &'a HashMap<PathBuf, LocalCfg>,
    test_root: &Path,
    file: &Path,
) -> Vec<&'a LocalCfg> {
    let mut v = Vec::new();
    let mut dir = file.parent();
    while let Some(d) = dir {
        if let Some(c) = cache.get(d) {
            v.push(c);
        }
        if d == test_root {
            break;
        }
        dir = d.parent();
    }
    v.reverse();
    v
}

fn load_local_cfgs(test_root: &Path, tests: &[(PathBuf, String)]) -> HashMap<PathBuf, LocalCfg> {
    let mut cache = HashMap::new();
    for (p, _) in tests {
        let mut dir = p.parent();
        while let Some(d) = dir {
            if !cache.contains_key(d)
                && let Ok(text) = std::fs::read_to_string(d.join("lit.local.cfg"))
            {
                cache.insert(d.to_path_buf(), LocalCfg::parse(&text));
            }
            if d == test_root {
                break;
            }
            dir = d.parent();
        }
    }
    cache
}

/// Runs the selection; `Ok(true)` when any test failed the run.
#[allow(clippy::too_many_lines)] // linear setup / run / report sequence
fn execute(opts: &Options) -> Result<bool, String> {
    let start = Instant::now();
    let root = repo_root();
    let tag = resolve_tag(
        opts.tag.as_deref(),
        opts.gate.as_deref(),
        std::env::var(TAG_ENV).ok().as_deref(),
    )?;
    let toolchain = match tag_major(&tag).filter(|m| crate::oracle::SUPPORTED.contains(m)) {
        Some(m) => crate::oracle::Oracle::parse(Some(&m.to_string()))?,
        None => crate::oracle::Oracle::from_env()?,
    };
    let checkout = opts.tests.clone().unwrap_or_else(|| default_checkout(&root, &tag));
    let test_root = checkout.join("libcxx").join("test");
    if !test_root.join("support").join("test_macros.h").is_file() {
        return Err(format!(
            "libc++ tests not found at {}; run scripts/fetch-libcxx-tests.sh {tag}",
            checkout.display()
        ));
    }
    let libcxx = if opts.list { None } else { libcxx_install(&root, toolchain)? };
    let roots: Vec<String> = if opts.dirs.is_empty() || opts.gate.is_some() {
        let mut r = gate_dirs(opts.gate.as_deref().unwrap_or("m3"));
        r.extend(opts.dirs.iter().cloned());
        r
    } else {
        opts.dirs.clone()
    };
    let tests = discover(&test_root, &roots, opts.filter.as_deref())?;
    if tests.is_empty() {
        return Err("no tests selected".into());
    }
    let cfg_cache = load_local_cfgs(&test_root, &tests);
    let features = features::detect(&toolchain.libcxx_include());
    let libcxx_version = features::libcxx_version(&toolchain.libcxx_include());
    let feature_refs: Vec<&str> = features.iter().map(String::as_str).collect();

    if opts.list {
        let target = target_dir(&root)?;
        for (p, rel) in &tests {
            let kind =
                p.file_name().and_then(|n| n.to_str()).and_then(Kind::of).unwrap_or(Kind::Pass);
            let text = std::fs::read_to_string(p).unwrap_or_default();
            let verdict = match run::env_unsupported(rel, &target, libcxx_version).map_or_else(
                || run::prepare(kind, &text, &local_cfgs(&cfg_cache, &test_root, p), &feature_refs),
                |r| Err(Outcome::Unsupported(r)),
            ) {
                Ok((d, _, xfail)) => {
                    let mut v = "run".to_owned();
                    if let Some(x) = xfail {
                        let _ = write!(v, " (XFAIL: {x})");
                    }
                    if let Some(why) = run::is_long(rel, &d) {
                        let _ = write!(v, " (long: {why})");
                    }
                    if d.allow_retries > 0 {
                        let _ = write!(v, " (ALLOW_RETRIES: {})", d.allow_retries);
                    }
                    v
                }
                Err(o) => o.to_string(),
            };
            println!("{:<13} {rel}  {verdict}", kind.label());
        }
        println!("{} tests", tests.len());
        return Ok(false);
    }

    let target = target_dir(&root)?;
    let kiln = match &opts.compiler {
        Some(c) => c.clone(),
        None => build_kiln_with(&root, &target, "libcxx", opts.profile)?,
    };
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.subsec_nanos());
    let scratch = target.join("xtask-libcxx").join(format!("run-{}-{nanos}", std::process::id()));
    std::fs::create_dir_all(&scratch)
        .map_err(|e| format!("cannot create {}: {e}", scratch.display()))?;
    let probe = RssProbe::detect(&scratch);
    let cfg = Config {
        kiln: kiln.clone(),
        checkout: checkout.clone(),
        tag,
        llvm_dir: toolchain.llvm_dir(),
        clang_compat: toolchain.version,
        libcxx,
        resource_dir: root.join("resource"),
        features: features.clone(),
        libcxx_version,
        compile_timeout: opts.compile_timeout(),
        run_timeout: opts.run_timeout,
        long_run_timeout: opts.long_run_timeout,
        mem: MemLimit::from_mib(opts.mem_limit_mib),
        probe,
        scratch: scratch.clone(),
        keep: opts.keep,
    };
    let jobs: Vec<Job<'_>> = tests
        .iter()
        .enumerate()
        .map(|(index, (p, rel))| Job {
            path: p.clone(),
            rel: rel.clone(),
            local_cfgs: local_cfgs(&cfg_cache, &test_root, p),
            index,
        })
        .collect();
    println!(
        "xtask libcxx: {} tests from {} with {} ({}, compile timeout {} s, -j {}{})",
        jobs.len(),
        test_root.display(),
        kiln.display(),
        profile_text(opts),
        cfg.compile_timeout.as_secs_f64(),
        opts.jobs,
        crate::jobs::global().describe().map_or_else(String::new, |d| format!(", jobserver {d}")),
    );
    let results = run_all(&cfg, &jobs, opts.jobs, opts.verbose);
    if !opts.keep {
        let _ = std::fs::remove_dir_all(&scratch);
    }
    let wall = start.elapsed();

    let clusters = report::clusters_of(&results);
    println!("\nTop clusters:");
    for (k, c) in clusters.ranked().into_iter().take(10) {
        println!("  {:>4} first / {:>4} any  {k}  (e.g. {})", c.first, c.any, c.example);
    }
    let mut total = report::Counts::default();
    for r in &results {
        total.add(&r.outcome);
    }
    println!(
        "\n{}; wall {:.1} s; {} kiln",
        total.summary_line(),
        wall.as_secs_f64(),
        profile_text(opts)
    );

    if let Some(path) = &opts.report {
        let info = run_info(opts, &cfg, &root, wall);
        let region = report::render(&info, &roots, &results, 40);
        let existing = std::fs::read_to_string(path).unwrap_or_default();
        std::fs::write(path, report::splice(&existing, &region))
            .map_err(|e| format!("cannot write {}: {e}", path.display()))?;
        println!("xtask libcxx: wrote {}", path.display());
    }
    Ok(results.iter().any(|r| r.outcome.is_failure()))
}

fn run_all(cfg: &Config, jobs: &[Job<'_>], n: usize, verbose: bool) -> Vec<TestResult> {
    let next = AtomicUsize::new(0);
    let done = AtomicUsize::new(0);
    let slots: Mutex<Vec<Option<TestResult>>> = Mutex::new(vec![None; jobs.len()]);
    std::thread::scope(|s| {
        for _ in 0..n.max(1) {
            s.spawn(|| {
                loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    let Some(job) = jobs.get(i) else { break };
                    let r = run::run_test(cfg, job);
                    let k = done.fetch_add(1, Ordering::Relaxed) + 1;
                    let quiet = matches!(
                        r.outcome,
                        Outcome::Pass | Outcome::Unsupported(_) | Outcome::XFail
                    );
                    if verbose || !quiet {
                        let first =
                            r.keys.first().map(|k| format!(" — {}", k.0)).unwrap_or_default();
                        println!("[{k}/{}] {}: {}{first}", jobs.len(), r.outcome, r.rel);
                        if verbose && !r.detail.is_empty() {
                            println!("{}", indent(&r.detail));
                        }
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
        .map(|r| r.expect("every test produced a result"))
        .collect()
}

fn indent(s: &str) -> String {
    s.lines().map(|l| format!("    {l}")).collect::<Vec<_>>().join("\n")
}

/// The profile as the status rows name it: `harness`; for a `--compiler`
/// binary the profile directory it sits in (`release (--compiler)` for
/// `target/release/kiln++`), else just `--compiler`.
fn profile_text(opts: &Options) -> String {
    let Some(c) = &opts.compiler else { return opts.profile.label().to_owned() };
    c.parent()
        .and_then(Path::file_name)
        .and_then(|d| d.to_str())
        .and_then(|d| Profile::parse(d).ok().filter(|p| p.dir() == d))
        .map_or_else(|| "--compiler".to_owned(), |p| format!("{} (--compiler)", p.label()))
}

fn run_info(opts: &Options, cfg: &Config, root: &Path, wall: Duration) -> report::RunInfo {
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
    let kiln_version =
        crate::util::identify(&cfg.kiln).map_or_else(|_| "unknown".to_owned(), |id| id.version);
    let mut command = "cargo xtask libcxx".to_owned();
    let gate = opts.gate.as_deref().unwrap_or("m3");
    if cfg.tag != gate_tag(gate) {
        let _ = write!(command, " --tag {}", cfg.tag);
    }
    if opts.gate.is_some() || opts.dirs.is_empty() {
        let _ = write!(command, " --gate {gate}");
    }
    for d in &opts.dirs {
        command.push(' ');
        command.push_str(d);
    }
    if let Some(f) = &opts.filter {
        let _ = write!(command, " --filter {f}");
    }
    if opts.compiler.is_none() && opts.profile != Profile::Harness {
        let _ = write!(command, " --profile {}", opts.profile.label());
    }
    if let Some(t) = opts.compile_timeout {
        let _ = write!(command, " --compile-timeout {}", t.as_secs_f64());
    }
    let defaults = Options::default();
    if opts.run_timeout != defaults.run_timeout {
        let _ = write!(command, " --timeout {}", opts.run_timeout.as_secs_f64());
    }
    if opts.long_run_timeout != defaults.long_run_timeout {
        let _ = write!(command, " --run-timeout-long {}", opts.long_run_timeout.as_secs_f64());
    }
    let _ = write!(command, " -j {}", opts.jobs);
    // Paths are written relative to the checkouts, so the page does not
    // name the machine: the tests checkout, its `.toolchain` directory
    // (which holds an extracted libc++ tree), then the repo.
    let shorten = |f: &str| {
        let toolchain_dir = cfg.checkout.parent().map(Path::to_path_buf).unwrap_or_default();
        let subst = [
            (&cfg.checkout, "<llvm-project>"),
            (&toolchain_dir, "<toolchain>"),
            (&root.to_path_buf(), "<repo>"),
        ];
        for (base, name) in subst {
            if base.as_os_str().is_empty() {
                continue;
            }
            if let Some(i) = f.find(&base.display().to_string()) {
                let rest = &f[i + base.display().to_string().len()..];
                return format!("{}{name}{rest}", &f[..i]);
            }
        }
        f.to_owned()
    };
    let flags: Vec<String> = cfg.base_compile_flags().iter().map(|f| shorten(f)).collect();
    let link: Vec<String> = cfg.link_flags().iter().map(|f| shorten(f)).collect();
    report::RunInfo {
        kiln_version,
        profile: profile_text(opts),
        kiln_commit: git(root, &["rev-parse", "--short", "HEAD"]),
        tests_tag: match git(&cfg.checkout, &["describe", "--tags", "--always"]) {
            t if t == cfg.tag => t,
            t => format!("{t} (expected {})", cfg.tag),
        },
        libcpp_version: libcpp_version(cfg),
        target: features::TARGET.to_owned(),
        command,
        compile_flags: format!("{} <ADDITIONAL_COMPILE_FLAGS> <test>", flags.join(" ")),
        link_flags: format!("t.o -o t.exe {}", link.join(" ")),
        features: cfg.features.iter().map(|f| format!("`{f}`")).collect::<Vec<_>>().join(", "),
        limits: format!(
            "{} per process (kiln: plus its stack reservation); compile/link timeout {} s; run timeout {} s ({} s for long tests); -j {}",
            cfg.mem.map_or_else(
                || "no memory cap".to_owned(),
                |m| format!("{} MiB address space", m.mib)
            ),
            cfg.compile_timeout.as_secs(),
            cfg.run_timeout.as_secs_f64(),
            cfg.long_run_timeout.as_secs_f64(),
            opts.jobs
        ),
        rss_source: cfg.probe.source_id().to_owned(),
        wall,
    }
}

/// `_LIBCPP_VERSION` as kiln's preprocessor sees it.
fn libcpp_version(cfg: &Config) -> String {
    let dir = cfg.scratch.with_extension("version");
    let _ = std::fs::create_dir_all(&dir);
    let src = dir.join("v.cpp");
    if std::fs::write(&src, "#include <version>\n").is_err() {
        return "unknown".to_owned();
    }
    let mut cmd = Command::new(&cfg.kiln);
    cmd.args(cfg.base_compile_flags())
        .args(["-E", "-dM"])
        .arg(&src)
        .env("TMPDIR", &dir)
        .env("KILN_LLVM_DIR", &cfg.llvm_dir);
    let out = process::run(&mut process::limited(&cmd, cfg.mem), cfg.compile_timeout);
    let _ = std::fs::remove_file(&src);
    let _ = std::fs::remove_dir(&dir);
    out.ok()
        .and_then(|o| {
            o.stdout_str().lines().find_map(|l| {
                l.strip_prefix("#define _LIBCPP_VERSION ").map(|v| v.trim().to_owned())
            })
        })
        .unwrap_or_else(|| "unknown".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Option<Options>, String> {
        parse_args(args.iter().map(|s| (*s).to_owned()))
    }

    #[test]
    fn options() {
        let o = parse(&[
            "--gate",
            "m3",
            "-j",
            "3",
            "--timeout=5",
            "--report",
            "p.md",
            "--keep",
            "std/numerics/bit/",
        ])
        .unwrap()
        .unwrap();
        assert_eq!(o.gate.as_deref(), Some("m3"));
        assert_eq!(o.jobs, 3);
        assert_eq!(o.run_timeout, Duration::from_secs(5));
        assert_eq!(o.report, Some(PathBuf::from("p.md")));
        assert!(o.keep);
        assert_eq!(o.dirs, ["std/numerics/bit"]);
        assert_eq!(parse(&["-h"]), Ok(None));
        assert!(parse(&["--gate", "m9"]).is_err());
        assert!(parse(&["--keep=1"]).is_err());
        assert!(parse(&["-j", "0"]).is_err());
        assert!(parse(&["--bogus"]).is_err());
        assert_eq!(Options::default().jobs, 2);
        assert_eq!(Options::default().mem_limit_mib, 8192);
    }

    #[test]
    fn discovery_and_local_cfgs() {
        let base = std::env::temp_dir().join(format!("kiln-libcxx-disc-{}", std::process::id()));
        let root = base.join("libcxx").join("test");
        let d = root.join("std").join("x");
        std::fs::create_dir_all(d.join("Inputs")).unwrap();
        std::fs::create_dir_all(d.join("sub")).unwrap();
        for f in [
            "a.pass.cpp",
            "b.verify.cpp",
            "helper.h",
            "Inputs/c.pass.cpp",
            "sub/d.compile.pass.cpp",
        ] {
            std::fs::write(d.join(f), "").unwrap();
        }
        std::fs::write(
            d.join("sub").join("lit.local.cfg"),
            "if \"no-x\" in config.available_features:\n  config.unsupported = True\n",
        )
        .unwrap();
        let tests = discover(&root, &["std/x".to_owned()], None).unwrap();
        let rels: Vec<&str> = tests.iter().map(|(_, r)| r.as_str()).collect();
        assert_eq!(
            rels,
            ["std/x/a.pass.cpp", "std/x/b.verify.cpp", "std/x/sub/d.compile.pass.cpp"]
        );
        let filtered = discover(&root, &["std/x".to_owned()], Some("sub/")).unwrap();
        assert_eq!(filtered.len(), 1);
        assert!(discover(&root, &["std/missing".to_owned()], None).is_err());
        let cache = load_local_cfgs(&root, &tests);
        assert_eq!(cache.len(), 1);
        assert_eq!(local_cfgs(&cache, &root, &tests[0].0).len(), 0);
        assert_eq!(local_cfgs(&cache, &root, &tests[2].0).len(), 1);
        std::fs::remove_dir_all(&base).unwrap();
    }

    /// `--profile` picks the implicit build and the default compile limit;
    /// `--compile-timeout` overrides it (spec/libcxx.md "Build profiles").
    #[test]
    fn profiles_and_compile_timeouts() {
        let d = parse(&[]).unwrap().unwrap();
        assert_eq!(d.profile, Profile::Harness);
        assert_eq!(d.compile_timeout(), Duration::from_secs(120));
        assert_eq!(profile_text(&d), "harness");
        let dbg = parse(&["--profile", "debug"]).unwrap().unwrap();
        assert_eq!(dbg.profile, Profile::Debug);
        assert_eq!(dbg.profile.cargo_name(), "dev");
        assert_eq!(dbg.profile.dir(), "debug");
        assert_eq!(dbg.compile_timeout(), Duration::from_secs(300));
        let rel = parse(&["--profile=release", "--compile-timeout", "45"]).unwrap().unwrap();
        assert_eq!(rel.profile, Profile::Release);
        assert_eq!(rel.compile_timeout(), Duration::from_secs(45));
        assert!(parse(&["--profile", "bench"]).is_err());
        assert!(parse(&["--compile-timeout", "0"]).is_err());
        assert!(parse(&["--compile-timeout", "x"]).is_err());
        let given = |p: &str| {
            profile_text(&Options { compiler: Some(PathBuf::from(p)), ..Options::default() })
        };
        assert_eq!(given("/t/release/kiln++"), "release (--compiler)");
        assert_eq!(given("/t/debug/kiln++"), "debug (--compiler)");
        assert_eq!(given("/usr/bin/kiln++"), "--compiler");
        assert_eq!(given("/t/dev/kiln++"), "--compiler");
    }

    /// `--tag`/`KILN_LIBCXX_TAG` pick the checkout (spec/libcxx.md "Setup").
    #[test]
    fn tag_selection() {
        assert_eq!(
            parse(&["--tag", "llvmorg-22.1.8"]).unwrap().unwrap().tag.as_deref(),
            Some("llvmorg-22.1.8")
        );
        assert_eq!(parse(&[]).unwrap().unwrap().tag, None);
        for bad in ["22.1.8", "llvmorg-", "llvmorg-../x", "llvmorg-1 2"] {
            assert!(parse(&["--tag", bad]).is_err(), "{bad}");
        }
        assert_eq!(resolve_tag(None, None, None).unwrap(), TESTS_TAG);
        assert_eq!(resolve_tag(None, None, Some("")).unwrap(), TESTS_TAG);
        assert_eq!(resolve_tag(None, None, Some("llvmorg-22.1.8")).unwrap(), "llvmorg-22.1.8");
        assert_eq!(
            resolve_tag(Some("llvmorg-20.1.0"), None, Some("llvmorg-22.1.8")).unwrap(),
            "llvmorg-20.1.0"
        );
        assert!(resolve_tag(None, None, Some("banana")).is_err());
        // A gate pins its release over the environment; --tag beats both.
        assert_eq!(resolve_tag(None, Some("m3"), Some("llvmorg-22.1.8")).unwrap(), TESTS_TAG);
        assert_eq!(resolve_tag(None, Some("m4"), None).unwrap(), M4_TAG);
        assert_eq!(
            resolve_tag(Some("llvmorg-22.1.0"), Some("m4"), None).unwrap(),
            "llvmorg-22.1.0"
        );
        assert_eq!(tag_major(TESTS_TAG), Some(19));
        assert_eq!(tag_major(M4_TAG), Some(22));
        assert_eq!(tag_major("llvmorg-23-init"), Some(23));
        assert_eq!(tag_major("llvmorg-x"), None);
    }

    /// The M4 gate is the M3 gate plus the M4 exit headers' directories,
    /// none of them twice (docs/ROADMAP.md M4).
    #[test]
    fn gates() {
        assert_eq!(gate_dirs("m3"), GATE_M3);
        let m4 = gate_dirs("m4");
        assert_eq!(&m4[..GATE_M3.len()], GATE_M3);
        assert_eq!(m4.len(), GATE_M3.len() + GATE_M4_EXTRA.len());
        let mut sorted = m4.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), m4.len());
        assert_eq!(parse(&["--gate", "m4"]).unwrap().unwrap().gate.as_deref(), Some("m4"));
    }

    /// Two tags' checkouts coexist under `.toolchain/libcxx-<tag>`; the
    /// default tag also finds the pre-tag `.toolchain/llvm-project`.
    #[test]
    fn checkouts_coexist() {
        let base = std::env::temp_dir().join(format!("kiln-libcxx-tags-{}", std::process::id()));
        let tc = base.join(".toolchain");
        std::fs::create_dir_all(&tc).unwrap();
        let t22 = "llvmorg-22.1.8";
        // Nothing fetched: the tagged path (the fetch script's target).
        assert_eq!(default_checkout(&base, t22), tc.join(format!("libcxx-{t22}")));
        assert_eq!(default_checkout(&base, TESTS_TAG), tc.join(format!("libcxx-{TESTS_TAG}")));
        // The legacy location serves the default tag only.
        std::fs::create_dir_all(tc.join("llvm-project")).unwrap();
        assert_eq!(default_checkout(&base, TESTS_TAG), tc.join("llvm-project"));
        assert_eq!(default_checkout(&base, t22), tc.join(format!("libcxx-{t22}")));
        // Tagged checkouts win, side by side.
        std::fs::create_dir_all(tc.join(format!("libcxx-{TESTS_TAG}"))).unwrap();
        std::fs::create_dir_all(tc.join(format!("libcxx-{t22}"))).unwrap();
        assert_eq!(default_checkout(&base, TESTS_TAG), tc.join(format!("libcxx-{TESTS_TAG}")));
        assert_eq!(default_checkout(&base, t22), tc.join(format!("libcxx-{t22}")));
        std::fs::remove_dir_all(&base).unwrap();
    }
}
