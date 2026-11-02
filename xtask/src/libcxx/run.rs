//! Running one libc++ test with kiln (`spec/libcxx.md` "Commands" and
//! "Outcomes").

use std::fmt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use super::cluster;
use super::directives::{Directives, Kind, LocalCfg};
use super::features;
use crate::util::process::{self, Measured, MemLimit, RssProbe};

/// Everything a test run needs that does not vary per test.
pub struct Config {
    pub kiln: PathBuf,
    /// The `llvm-project` checkout (holding `libcxx/test`).
    pub checkout: PathBuf,
    /// The release tag the checkout should be at (`--tag`).
    pub tag: String,
    /// `/usr/lib/llvm-N` of `KILN_ORACLE`: kiln++ runs with it as
    /// `KILN_LLVM_DIR` (`spec/harness.md` "Oracle toolchain").
    pub llvm_dir: PathBuf,
    /// kiln++'s `-fclang-compat`: the major of `llvm_dir`, so kiln
    /// identifies as the clang whose libc++ it compiles. 19 is kiln's
    /// default and is not passed.
    pub clang_compat: u32,
    /// The libc++ to use when it is not `llvm_dir`'s own.
    pub libcxx: Option<LibcxxTree>,
    pub resource_dir: PathBuf,
    pub features: Vec<String>,
    pub compile_timeout: Duration,
    pub run_timeout: Duration,
    /// The run limit of a long test ([`is_long`]): `--run-timeout-long`.
    pub long_run_timeout: Duration,
    pub mem: Option<MemLimit>,
    pub probe: RssProbe,
    /// The per-run scratch root; test `i` uses `<root>/t<i>`.
    pub scratch: PathBuf,
    pub keep: bool,
}

/// An extracted libc++ package tree (spec/libcxx.md "Toolchain"): its
/// headers replace the system's (`-nostdinc++ -isystem`) and its static
/// archives are linked, so no shared libc++ of another release is picked up.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LibcxxTree {
    /// `.../include/c++/v1`.
    pub include: PathBuf,
    /// The directory of `libc++.a`, `libc++abi.a`, `libc++experimental.a`.
    pub lib: PathBuf,
}

impl Config {
    pub fn test_root(&self) -> PathBuf {
        self.checkout.join("libcxx").join("test")
    }

    fn feature_refs(&self) -> Vec<&str> {
        self.features.iter().map(String::as_str).collect()
    }

    /// The flags every compile gets, before the test's own.
    pub fn base_compile_flags(&self) -> Vec<String> {
        let mut v: Vec<String> = [
            format!("-std={}", features::STD),
            "-stdlib=libc++".to_owned(),
            "-resource-dir".to_owned(),
            self.resource_dir.display().to_string(),
            "-pthread".to_owned(),
            "-ferror-limit=0".to_owned(),
            "-D_LIBCPP_ENABLE_EXPERIMENTAL".to_owned(),
        ]
        .into();
        if self.clang_compat != 19 {
            v.push(format!("-fclang-compat={}", self.clang_compat));
        }
        if let Some(t) = &self.libcxx {
            v.extend(["-nostdinc++".to_owned(), "-isystem".to_owned()]);
            v.push(t.include.display().to_string());
        }
        v.push("-I".to_owned());
        v.push(self.test_root().join("support").display().to_string());
        v
    }

    /// The flags of the link step, after the object. With a package tree,
    /// its archives are linked statically and `--as-needed` drops the
    /// driver's own trailing `-lc++`.
    pub fn link_flags(&self) -> Vec<String> {
        let mut v: Vec<String> = ["-stdlib=libc++", "-pthread"].map(str::to_owned).into();
        match &self.libcxx {
            None => v.extend(["-lc++experimental", "-lc++abi"].map(str::to_owned)),
            Some(t) => {
                v.push(format!("-L{}", t.lib.display()));
                v.extend(
                    [
                        "-Wl,--as-needed",
                        "-Wl,-Bstatic",
                        "-lc++experimental",
                        "-lc++",
                        "-lc++abi",
                        "-Wl,-Bdynamic",
                    ]
                    .map(str::to_owned),
                );
            }
        }
        v
    }
}

/// The flags of a `.verify.cpp` compile: libc++'s `%{verify}`
/// substitution (`libcxx/utils/libcxx/test/params.py`), outcome from the
/// exit status (spec/libcxx.md ".verify.cpp").
pub const VERIFY_FLAGS: &[&str] = &[
    "-fsyntax-only",
    "-Wno-error",
    "-Xclang",
    "-verify",
    "-Xclang",
    "-verify-ignore-unexpected=note",
];

/// The step a test failed in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Stage {
    Compile,
    Link,
    Run,
    /// A `.compile.fail` test compiled, or a `.link.fail` test linked.
    ExpectedFail,
}

impl Stage {
    pub fn label(self) -> &'static str {
        match self {
            Self::Compile => "compile",
            Self::Link => "link",
            Self::Run => "run",
            Self::ExpectedFail => "expected-fail-but-succeeded",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    Pass,
    Fail(Stage),
    XFail,
    XPass,
    Unsupported(String),
    Killed(Stage),
    Timeout(Stage),
    Error(String),
}

impl Outcome {
    pub fn tag(&self) -> &'static str {
        match self {
            Self::Pass => "PASS",
            Self::Fail(_) => "FAIL",
            Self::XFail => "XFAIL",
            Self::XPass => "XPASS",
            Self::Unsupported(_) => "UNSUPPORTED",
            Self::Killed(_) => "KILLED",
            Self::Timeout(_) => "TIMEOUT",
            Self::Error(_) => "ERROR",
        }
    }

    /// Whether the outcome fails the run (exit status 1).
    pub fn is_failure(&self) -> bool {
        matches!(
            self,
            Self::Fail(_) | Self::XPass | Self::Killed(_) | Self::Timeout(_) | Self::Error(_)
        )
    }
}

impl fmt::Display for Outcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Fail(s) | Self::Killed(s) | Self::Timeout(s) => {
                write!(f, "{} ({})", self.tag(), s.label())
            }
            Self::Unsupported(r) | Self::Error(r) => write!(f, "{} ({r})", self.tag()),
            _ => f.write_str(self.tag()),
        }
    }
}

/// The result of one test.
#[derive(Clone, Debug)]
pub struct TestResult {
    /// Path relative to `libcxx/test`, with `/` separators.
    pub rel: String,
    pub outcome: Outcome,
    /// Stage-prefixed cluster keys with a raw example each (first error
    /// first); empty unless a step failed.
    pub keys: Vec<(String, String)>,
    /// kiln panicked or crashed (never hidden by `XFAIL`).
    pub crashed: bool,
    /// The tail of the failing step's stderr.
    pub detail: String,
    pub wall: Duration,
    /// The largest per-process peak RSS among the test's steps, KiB.
    pub max_rss_kb: Option<u64>,
}

/// One test to run: its path, the lit.local.cfg files above it (outermost
/// first), and its index (for the scratch directory).
pub struct Job<'a> {
    pub path: PathBuf,
    pub rel: String,
    pub local_cfgs: Vec<&'a LocalCfg>,
    pub index: usize,
}

/// Tests this host cannot run as libc++'s CI does: built with clang 19
/// they fail the same way (spec/libcxx.md "Environment exclusions").
/// Paths relative to `libcxx/test`, sorted.
///
/// Run as root (the agent containers), a test that expects `permissions(
/// dir, perms::none)` (or a mode-0 file) to make a path inaccessible gets
/// no `EACCES`: root bypasses permission checks.
const ROOT_UNSUPPORTED: &[&str] = &[
    "std/input.output/filesystems/class.directory_entry/directory_entry.cons/path.pass.cpp",
    "std/input.output/filesystems/class.directory_entry/directory_entry.mods/assign.pass.cpp",
    "std/input.output/filesystems/class.directory_entry/directory_entry.mods/refresh.pass.cpp",
    "std/input.output/filesystems/class.directory_entry/directory_entry.mods/replace_filename.pass.cpp",
    "std/input.output/filesystems/class.directory_entry/directory_entry.obs/file_size.pass.cpp",
    "std/input.output/filesystems/class.directory_entry/directory_entry.obs/file_type_obs.pass.cpp",
    "std/input.output/filesystems/class.directory_entry/directory_entry.obs/hard_link_count.pass.cpp",
    "std/input.output/filesystems/class.directory_entry/directory_entry.obs/last_write_time.pass.cpp",
    "std/input.output/filesystems/class.rec.dir.itr/rec.dir.itr.members/increment.pass.cpp",
    "std/input.output/filesystems/fs.op.funcs/fs.op.exists/exists.pass.cpp",
    "std/input.output/filesystems/fs.op.funcs/fs.op.is_block_file/is_block_file.pass.cpp",
    "std/input.output/filesystems/fs.op.funcs/fs.op.is_char_file/is_character_file.pass.cpp",
    "std/input.output/filesystems/fs.op.funcs/fs.op.is_directory/is_directory.pass.cpp",
    "std/input.output/filesystems/fs.op.funcs/fs.op.is_empty/is_empty.pass.cpp",
    "std/input.output/filesystems/fs.op.funcs/fs.op.is_fifo/is_fifo.pass.cpp",
    "std/input.output/filesystems/fs.op.funcs/fs.op.is_other/is_other.pass.cpp",
    "std/input.output/filesystems/fs.op.funcs/fs.op.is_regular_file/is_regular_file.pass.cpp",
    "std/input.output/filesystems/fs.op.funcs/fs.op.is_socket/is_socket.pass.cpp",
    "std/input.output/filesystems/fs.op.funcs/fs.op.is_symlink/is_symlink.pass.cpp",
    "std/input.output/filesystems/fs.op.funcs/fs.op.last_write_time/last_write_time.pass.cpp",
    "std/input.output/filesystems/fs.op.funcs/fs.op.status/status.pass.cpp",
    "std/input.output/filesystems/fs.op.funcs/fs.op.symlink_status/symlink_status.pass.cpp",
    "std/input.output/filesystems/fs.op.funcs/fs.op.temp_dir_path/temp_directory_path.pass.cpp",
];

/// The UNSUPPORTED reason of a [`ROOT_UNSUPPORTED`] test run as root.
pub const ROOT_REASON: &str = "env: running as root: perms::none does not deny access";

/// With the scratch directory (`TMPDIR`) on a host-shared filesystem (a
/// Docker Desktop bind mount): a directory's hard-link count is not
/// `2 + subdirectories`, and `create_directory(p, attrs)` fails copying
/// the attributes.
const HOST_FS_UNSUPPORTED: &[&str] = &[
    "std/input.output/filesystems/fs.op.funcs/fs.op.create_directory/create_directory_with_attributes.pass.cpp",
    "std/input.output/filesystems/fs.op.funcs/fs.op.hard_lk_ct/hard_link_count.pass.cpp",
];

/// With glibc locale data whose `en_US.UTF-8` digit grouping is `3` (glibc
/// 2.41 here; `LC_ALL=en_US.UTF-8 locale -k grouping`), not the `3;3`
/// these tests expect on every non-Windows, non-AIX target.
const GROUPING_UNSUPPORTED: &[&str] = &[
    "std/localization/locale.categories/category.monetary/locale.moneypunct.byname/grouping.pass.cpp",
    "std/localization/locale.categories/facet.numpunct/locale.numpunct.byname/grouping.pass.cpp",
];

/// The UNSUPPORTED reason of a [`GROUPING_UNSUPPORTED`] test.
pub const GROUPING_REASON: &str = "env: glibc en_US.UTF-8 grouping is 3, the test expects 3;3";

/// Filesystem types of a host-shared bind mount (`/proc/self/mountinfo`).
const HOST_FS_TYPES: &[&str] = &["fakeowner", "fuse.grpcfuse", "virtiofs"];

/// Why `rel` is UNSUPPORTED on this host, if it is; `scratch` is where its
/// scratch directory lives.
pub fn env_unsupported(rel: &str, scratch: &Path) -> Option<String> {
    if ROOT_UNSUPPORTED.contains(&rel) && running_as_root() {
        return Some(ROOT_REASON.to_owned());
    }
    if GROUPING_UNSUPPORTED.contains(&rel) && en_us_grouping_is_3() {
        return Some(GROUPING_REASON.to_owned());
    }
    if HOST_FS_UNSUPPORTED.contains(&rel)
        && let Some(fs) = mount_fs_type(scratch).filter(|t| HOST_FS_TYPES.contains(&t.as_str()))
    {
        return Some(format!("env: TMPDIR is on a host-shared {fs} mount"));
    }
    None
}

/// Whether this process's effective uid is 0 (`/proc/self/status`).
fn running_as_root() -> bool {
    static ROOT: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ROOT.get_or_init(|| {
        euid_is_root(&std::fs::read_to_string("/proc/self/status").unwrap_or_default())
    })
}

/// Whether glibc's `en_US.UTF-8` groups digits by 3 only: `locale -k
/// grouping` prints exactly `grouping=3` (`3;3` is what the tests want;
/// without the locale it prints C's `-1`).
fn en_us_grouping_is_3() -> bool {
    static G: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *G.get_or_init(|| {
        Command::new("locale")
            .args(["-k", "grouping"])
            .env("LC_ALL", "en_US.UTF-8")
            .output()
            .is_ok_and(|o| {
                o.status.success() && String::from_utf8_lossy(&o.stdout).trim() == "grouping=3"
            })
    })
}

/// Whether the effective uid (the second field of `Uid:`) in the text of
/// `/proc/<pid>/status` is 0.
fn euid_is_root(status: &str) -> bool {
    status
        .lines()
        .find_map(|l| l.strip_prefix("Uid:"))
        .and_then(|ids| ids.split_whitespace().nth(1))
        .is_some_and(|euid| euid == "0")
}

/// The filesystem type of the mount holding `path` (the longest mount
/// point above it in `/proc/self/mountinfo`), if known.
fn mount_fs_type(path: &Path) -> Option<String> {
    let path = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let info = std::fs::read_to_string("/proc/self/mountinfo").ok()?;
    mount_fs_type_in(&info, &path)
}

fn mount_fs_type_in(info: &str, path: &Path) -> Option<String> {
    let mut best: Option<(usize, &str)> = None;
    for line in info.lines() {
        let Some((pre, post)) = line.split_once(" - ") else { continue };
        let Some(point) = pre.split(' ').nth(4) else { continue };
        let Some(fs) = post.split(' ').next() else { continue };
        // Mount points escape space, tab, newline and backslash as `\ooo`.
        let point = point.replace("\\040", " ").replace("\\011", "\t").replace("\\134", "\\");
        if path.starts_with(&point) && best.is_none_or(|(n, _)| point.len() >= n) {
            best = Some((point.len(), fs));
        }
    }
    best.map(|(_, fs)| fs.to_owned())
}

/// `slow_tests.txt`: tests that get the long run timeout although they do
/// not say `REQUIRES: long_tests` (spec/libcxx.md "Timeouts and retries").
const SLOW_TESTS: &str = include_str!("slow_tests.txt");

/// The `(path, reason)` entries of [`SLOW_TESTS`], in file order.
fn slow_tests() -> impl Iterator<Item = (&'static str, &'static str)> {
    SLOW_TESTS.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')).map(|l| {
        let (path, reason) = l.split_once(char::is_whitespace).unwrap_or((l, ""));
        (path, reason.trim())
    })
}

/// Why `rel` gets the long run timeout, if it does: it says `REQUIRES:
/// long_tests`, or it is in `slow_tests.txt`.
pub fn is_long(rel: &str, d: &Directives) -> Option<&'static str> {
    if d.requires_feature("long_tests") {
        return Some("REQUIRES: long_tests");
    }
    slow_tests().find(|&(p, _)| p == rel).map(|_| "slow_tests.txt")
}

/// What lit decides before anything runs: `Err(outcome)` ends the test
/// (UNSUPPORTED or ERROR), `Ok` carries the directives, flags and the
/// XFAIL expression that applies, if any.
pub fn prepare(
    kind: Kind,
    text: &str,
    local_cfgs: &[&LocalCfg],
    features: &[&str],
) -> Result<(Directives, Vec<String>, Option<String>), Outcome> {
    for cfg in local_cfgs {
        if let Some(r) = cfg.unsupported(features) {
            return Err(Outcome::Unsupported(r));
        }
    }
    let d = Directives::parse(text).map_err(Outcome::Error)?;
    let xfail = d.verdict(features).map_err(Outcome::Unsupported)?;
    if let Some(r) = kind.unsupported() {
        return Err(Outcome::Unsupported(r.to_owned()));
    }
    if d.has_run {
        return Err(Outcome::Unsupported("driver: RUN lines".to_owned()));
    }
    if !d.module_dependencies.is_empty() {
        return Err(Outcome::Unsupported("driver: modules".to_owned()));
    }
    let flags = d.flags_for(features);
    if flags.iter().any(|f| f.contains("%{")) {
        return Err(Outcome::Unsupported("driver: substitution".to_owned()));
    }
    if d.file_dependencies.iter().any(|f| f.contains("%{") || f.contains('*')) {
        return Err(Outcome::Unsupported("driver: FILE_DEPENDENCIES".to_owned()));
    }
    Ok((d, flags, xfail))
}

/// A step's verdict.
enum Step {
    Ok,
    Failed { keys: Vec<(String, String)>, crashed: bool, detail: String },
    Killed(String),
    Timeout,
}

pub fn run_test(cfg: &Config, job: &Job<'_>) -> TestResult {
    let name = job.path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    let kind = Kind::of(name).unwrap_or(Kind::Pass);
    let mut r = TestResult {
        rel: job.rel.clone(),
        outcome: Outcome::Pass,
        keys: Vec::new(),
        crashed: false,
        detail: String::new(),
        wall: Duration::ZERO,
        max_rss_kb: None,
    };
    if let Some(reason) = env_unsupported(&job.rel, &cfg.scratch) {
        r.outcome = Outcome::Unsupported(reason);
        return r;
    }
    let text = match std::fs::read(&job.path) {
        Ok(b) => String::from_utf8_lossy(&b).into_owned(),
        Err(e) => {
            r.outcome = Outcome::Error(format!("cannot read: {e}"));
            return r;
        }
    };
    let features = cfg.feature_refs();
    let (d, flags, xfail) = match prepare(kind, &text, &job.local_cfgs, &features) {
        Ok(p) => p,
        Err(o) => {
            r.outcome = o;
            return r;
        }
    };
    let scratch = cfg.scratch.join(format!("t{}", job.index));
    if let Err(e) = std::fs::create_dir_all(&scratch) {
        r.outcome = Outcome::Error(format!("cannot create {}: {e}", scratch.display()));
        return r;
    }
    let dir = job.path.parent().unwrap_or(Path::new("."));
    for dep in &d.file_dependencies {
        let from = dir.join(dep);
        let to = scratch.join(Path::new(dep).file_name().unwrap_or_default());
        if std::fs::copy(&from, &to).is_err() {
            r.outcome = Outcome::Unsupported("driver: FILE_DEPENDENCIES".to_owned());
            cleanup(cfg, &scratch);
            return r;
        }
    }
    let run_timeout =
        if is_long(&job.rel, &d).is_some() { cfg.long_run_timeout } else { cfg.run_timeout };
    let run = RunLimits { timeout: run_timeout, retries: d.allow_retries };
    execute(cfg, &job.path, &scratch, kind, &flags, run, &mut r);
    // XFAIL turns PASS into XPASS and FAIL into XFAIL; it never hides a
    // KILLED, a TIMEOUT or a compiler crash.
    if xfail.is_some() && !r.crashed {
        r.outcome = match r.outcome {
            Outcome::Pass => Outcome::XPass,
            Outcome::Fail(_) => Outcome::XFail,
            o => o,
        };
    }
    cleanup(cfg, &scratch);
    r
}

fn cleanup(cfg: &Config, scratch: &Path) {
    if !cfg.keep {
        let _ = std::fs::remove_dir_all(scratch);
    }
}

/// How a test binary runs: its time limit, and how many times a failing
/// run is retried (`ALLOW_RETRIES:`).
#[derive(Clone, Copy, Debug)]
struct RunLimits {
    timeout: Duration,
    retries: u32,
}

/// Runs the steps of `kind` and fills in `r`'s outcome, keys and timing.
/// Only the run step is retried: compiling and linking are deterministic,
/// so lit's rerun of the whole test reaches the same binary.
fn execute(
    cfg: &Config,
    src: &Path,
    scratch: &Path,
    kind: Kind,
    flags: &[String],
    run: RunLimits,
    r: &mut TestResult,
) {
    let obj = scratch.join("t.o");
    let exe = scratch.join("t.exe");
    let compile = |extra: &[&str]| {
        let mut c = Command::new(&cfg.kiln);
        c.args(cfg.base_compile_flags()).args(flags).args(extra).arg(src);
        c.env("KILN_LLVM_DIR", &cfg.llvm_dir);
        c
    };
    let link = || {
        let mut c = Command::new(&cfg.kiln);
        c.arg(&obj).arg("-o").arg(&exe).args(cfg.link_flags());
        c.env("KILN_LLVM_DIR", &cfg.llvm_dir);
        c
    };
    let obj_s = obj.display().to_string();
    let steps: Vec<(Stage, Command, bool)> = match kind {
        Kind::CompilePass => vec![(Stage::Compile, compile(&["-fsyntax-only"]), true)],
        Kind::CompileFail => vec![(Stage::Compile, compile(&["-fsyntax-only"]), false)],
        // libc++'s `%{verify}` (`-ferror-limit=0` is in the base flags).
        Kind::Verify => vec![(Stage::Compile, compile(VERIFY_FLAGS), true)],
        Kind::LinkPass => vec![
            (Stage::Compile, compile(&["-c", "-o", &obj_s]), true),
            (Stage::Link, link(), true),
        ],
        Kind::LinkFail => vec![
            (Stage::Compile, compile(&["-c", "-o", &obj_s]), true),
            (Stage::Link, link(), false),
        ],
        _ => {
            let mut run = Command::new(&exe);
            run.current_dir(scratch);
            vec![
                (Stage::Compile, compile(&["-c", "-o", &obj_s]), true),
                (Stage::Link, link(), true),
                (Stage::Run, run, true),
            ]
        }
    };
    for (stage, mut cmd, expect_success) in steps {
        cmd.current_dir(scratch).env("TMPDIR", scratch);
        let is_kiln = stage != Stage::Run;
        let limit = if is_kiln {
            let args: Vec<_> = cmd.get_args().map(std::ffi::OsStr::to_os_string).collect();
            cfg.mem.map(|m| m.plus(process::kiln_stack_reservation_mib(&args)))
        } else {
            cfg.mem
        };
        let timeout = if is_kiln { cfg.compile_timeout } else { run.timeout };
        let attempts = if is_kiln { 1 } else { run.retries + 1 };
        let verdict =
            match attempt_step(cfg, &cmd, scratch, stage, kind, timeout, limit, attempts, r) {
                Ok(v) => v,
                Err(e) => {
                    r.outcome = Outcome::Error(format!(
                        "cannot run {stage:?} step: {e}",
                        stage = stage.label()
                    ));
                    return;
                }
            };
        match verdict {
            Step::Ok if expect_success => {}
            Step::Ok => {
                r.outcome = Outcome::Fail(Stage::ExpectedFail);
                let what = if stage == Stage::Compile { "compiled" } else { "linked" };
                r.keys =
                    vec![(format!("{}.fail: {what} successfully", stage.label()), String::new())];
                return;
            }
            Step::Failed { crashed: false, .. } if !expect_success => return,
            Step::Failed { keys, crashed, detail } => {
                r.outcome = Outcome::Fail(stage);
                r.keys = keys;
                r.crashed = crashed;
                r.detail = detail;
                return;
            }
            Step::Killed(detail) => {
                r.outcome = Outcome::Killed(stage);
                r.detail = detail;
                return;
            }
            Step::Timeout => {
                r.outcome = Outcome::Timeout(stage);
                r.detail = format!("killed after {}s", timeout.as_secs());
                return;
            }
        }
    }
}

/// Runs one step up to `attempts` times, until it succeeds, adding each
/// attempt's wall time and peak RSS to `r`; the last attempt's verdict.
#[allow(clippy::too_many_arguments)] // the step, its limits and the result
fn attempt_step(
    cfg: &Config,
    cmd: &Command,
    scratch: &Path,
    stage: Stage,
    kind: Kind,
    timeout: Duration,
    limit: Option<MemLimit>,
    attempts: u32,
    r: &mut TestResult,
) -> std::io::Result<Step> {
    let mut verdict = Step::Ok;
    for attempt in 1..=attempts {
        let m = process::run_measured(cmd, &cfg.probe, &scratch.join("rss.txt"), timeout, limit)?;
        r.wall += m.wall;
        r.max_rss_kb = r.max_rss_kb.max(m.max_rss_kb);
        verdict = judge(stage, &m, kind == Kind::Verify);
        if matches!(verdict, Step::Ok) {
            if attempt > 1 {
                r.detail = format!("passed on attempt {attempt} of {attempts}");
            }
            break;
        }
    }
    Ok(verdict)
}

/// Classifies one step's end. A compiler step "crashes" on a panic, an LLVM
/// crash banner or a signal; a test binary that aborts is an ordinary
/// failure.
fn judge(stage: Stage, m: &Measured, verify: bool) -> Step {
    let stderr = String::from_utf8_lossy(&m.stderr);
    if m.status.is_none() {
        return Step::Timeout;
    }
    let out = m.as_output();
    if process::out_of_memory(m.status, &stderr) {
        return Step::Killed(tail(&stderr, 6));
    }
    if out.success() {
        return Step::Ok;
    }
    let detail = tail(&stderr, 12);
    let signalled = out.code().is_none_or(|c| c >= 128);
    let label = stage.label();
    let verify_keys = if verify { cluster::verify_keys(&stderr) } else { Vec::new() };
    let errors = if verify_keys.is_empty() {
        cluster::staged(label, cluster::error_keys(&stderr))
    } else {
        verify_keys
    };
    if stage != Stage::Run {
        // A crash is the first key; the errors before it still count as
        // "any" occurrences of their clusters.
        let crash = cluster::crash_key(&stderr).or_else(|| {
            signalled.then(|| (format!("crash: {}", out.describe_end()), out.describe_end()))
        });
        if let Some(k) = crash {
            let mut keys = vec![k];
            keys.extend(errors);
            return Step::Failed { keys, crashed: true, detail };
        }
    }
    let mut keys = errors;
    if keys.is_empty()
        && stage == Stage::Link
        && let Some(l) = stderr.lines().find(|l| l.contains("undefined reference to"))
    {
        let i = l.find("undefined reference to").unwrap_or(0);
        keys.push((format!("link: {}", cluster::normalize(&l[i..])), l.trim().to_owned()));
    }
    if keys.is_empty()
        && stage == Stage::Run
        && let Some(l) = stderr.lines().find(|l| l.contains("Assertion") && l.contains("failed"))
    {
        keys.push(("run: assertion failed".to_owned(), l.trim().to_owned()));
    }
    if keys.is_empty() {
        let first = stderr.lines().map(str::trim).find(|l| !l.is_empty());
        let k = match first {
            Some(l) => format!("{label}: {}", cluster::normalize(l)),
            None => format!("{label}: {}", out.describe_end()),
        };
        keys.push((k, first.unwrap_or("").to_owned()));
    }
    Step::Failed { keys, crashed: false, detail }
}

/// The last `n` non-empty lines of `s`, each cut to 300 characters.
pub fn tail(s: &str, n: usize) -> String {
    let lines: Vec<&str> = s.lines().filter(|l| !l.trim().is_empty()).collect();
    let start = lines.len().saturating_sub(n);
    lines[start..]
        .iter()
        .map(|l| {
            if l.chars().count() > 300 {
                format!("{}…", l.chars().take(300).collect::<String>())
            } else {
                (*l).to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    const F: &[&str] = &["c++26", "kiln", "linux", "c++experimental"];

    #[test]
    fn preparation() {
        let p = |kind, text: &str| prepare(kind, text, &[], F).map(|(_, f, x)| (f, x));
        assert_eq!(p(Kind::Pass, "int main(){}"), Ok((vec![], None)));
        assert_eq!(
            p(Kind::Pass, "// UNSUPPORTED: c++26"),
            Err(Outcome::Unsupported("UNSUPPORTED: c++26".into()))
        );
        // A config verdict outranks the driver's.
        assert_eq!(
            p(Kind::Verify, "// REQUIRES: c++03"),
            Err(Outcome::Unsupported("REQUIRES: c++03".into()))
        );
        assert_eq!(p(Kind::Verify, ""), Ok((vec![], None)));
        assert_eq!(
            p(Kind::Pass, "// RUN: %{exec} x"),
            Err(Outcome::Unsupported("driver: RUN lines".into()))
        );
        assert_eq!(
            p(Kind::Pass, "// ADDITIONAL_COMPILE_FLAGS: %{maybe-include}"),
            Err(Outcome::Unsupported("driver: substitution".into()))
        );
        assert_eq!(
            p(Kind::Pass, "// MODULE_DEPENDENCIES: std"),
            Err(Outcome::Unsupported("driver: modules".into()))
        );
        assert_eq!(p(Kind::Pass, "// XFAIL: kiln"), Ok((vec![], Some("kiln".into()))));
        assert!(matches!(p(Kind::Pass, "// XFAIL: (a"), Err(Outcome::Error(_))));
        let cfg = LocalCfg::Conditions(vec![
            ("no-localization".into(), false),
            ("c++experimental".into(), true),
        ]);
        assert_eq!(prepare(Kind::Pass, "", &[&cfg], F).map(|_| ()), Ok(()));
        assert_eq!(
            prepare(Kind::Pass, "", &[&LocalCfg::Opaque], F).map(|_| ()),
            Err(Outcome::Unsupported("driver: lit.local.cfg".into()))
        );
    }

    #[test]
    fn outcomes() {
        assert!(Outcome::Fail(Stage::Run).is_failure());
        assert!(Outcome::XPass.is_failure());
        assert!(Outcome::Killed(Stage::Compile).is_failure());
        assert!(Outcome::Timeout(Stage::Run).is_failure());
        assert!(Outcome::Error(String::new()).is_failure());
        assert!(!Outcome::XFail.is_failure());
        assert!(!Outcome::Unsupported(String::new()).is_failure());
        assert_eq!(
            Outcome::Fail(Stage::ExpectedFail).to_string(),
            "FAIL (expected-fail-but-succeeded)"
        );
        assert_eq!(
            Outcome::Unsupported("driver: -verify".into()).to_string(),
            "UNSUPPORTED (driver: -verify)"
        );
    }

    #[test]
    fn env_exclusions() {
        for list in [ROOT_UNSUPPORTED, HOST_FS_UNSUPPORTED, GROUPING_UNSUPPORTED] {
            let mut sorted = list.to_vec();
            sorted.sort_unstable();
            sorted.dedup();
            assert_eq!(sorted, list);
        }
        let tmp = std::env::temp_dir();
        assert_eq!(env_unsupported("std/utilities/any/any.class/nothing.pass.cpp", &tmp), None);
        let listed = env_unsupported(ROOT_UNSUPPORTED[0], &tmp);
        assert_eq!(listed.is_some(), running_as_root());
        assert!(listed.is_none_or(|r| r == ROOT_REASON));
        let listed = env_unsupported(GROUPING_UNSUPPORTED[0], &tmp);
        assert_eq!(listed.is_some(), en_us_grouping_is_3());
        assert!(listed.is_none_or(|r| r == GROUPING_REASON));
        // The effective uid is the second `Uid:` field (real, effective,
        // saved, filesystem).
        assert!(euid_is_root("Name:\tx\nUid:\t1000\t0\t0\t0\nGid:\t0\t0\t0\t0\n"));
        assert!(!euid_is_root("Uid:\t0\t1000\t1000\t1000\n"));
        assert!(!euid_is_root("Name:\tx\n"));
    }

    #[test]
    fn slow_tests_list() {
        let entries: Vec<_> = slow_tests().collect();
        assert_ne!(entries.len(), 0);
        let mut sorted: Vec<&str> = entries.iter().map(|(p, _)| *p).collect();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted, entries.iter().map(|(p, _)| *p).collect::<Vec<_>>());
        for (path, reason) in &entries {
            assert!(path.starts_with("std/") || path.starts_with("libcxx/"), "{path}");
            assert!(Kind::of(path.rsplit('/').next().unwrap()).is_some(), "{path}");
            assert!(!reason.is_empty(), "{path} has no reason");
        }
        let plain = Directives::default();
        assert_eq!(is_long(entries[0].0, &plain), Some("slow_tests.txt"));
        assert_eq!(is_long("std/a/b.pass.cpp", &plain), None);
        let long = Directives::parse("// REQUIRES: long_tests").unwrap();
        assert_eq!(is_long("std/a/b.pass.cpp", &long), Some("REQUIRES: long_tests"));
    }

    /// A fake compiler: `-c -o t.o` writes the object, the link step writes
    /// `t.exe` as a copy of the test source (a shell script), so the run
    /// step runs the test's own script.
    #[cfg(unix)]
    fn fake_run(script: &str, run: RunLimits) -> TestResult {
        use std::os::unix::fs::PermissionsExt;
        let base = std::env::temp_dir().join(format!(
            "kiln-libcxx-retry-{}-{}",
            std::process::id(),
            script.len()
        ));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).unwrap();
        let kiln = base.join("fake-kiln");
        let src = base.join("t.pass.cpp");
        std::fs::write(
            &kiln,
            format!(
                "#!/bin/sh\nfor a; do case \"$a\" in -c) exec touch t.o;; esac; done\n\
                 cp {} t.exe && chmod +x t.exe\n",
                src.display()
            ),
        )
        .unwrap();
        std::fs::set_permissions(&kiln, std::fs::Permissions::from_mode(0o755)).unwrap();
        std::fs::write(&src, script).unwrap();
        let cfg = Config {
            kiln,
            checkout: base.clone(),
            tag: String::new(),
            llvm_dir: base.clone(),
            clang_compat: 19,
            libcxx: None,
            resource_dir: base.clone(),
            features: Vec::new(),
            compile_timeout: Duration::from_secs(20),
            run_timeout: run.timeout,
            long_run_timeout: run.timeout,
            mem: None,
            probe: RssProbe::Unavailable,
            scratch: base.join("s"),
            keep: false,
        };
        let scratch = base.join("s");
        std::fs::create_dir_all(&scratch).unwrap();
        let mut r = TestResult {
            rel: "t.pass.cpp".into(),
            outcome: Outcome::Pass,
            keys: Vec::new(),
            crashed: false,
            detail: String::new(),
            wall: Duration::ZERO,
            max_rss_kb: None,
        };
        execute(&cfg, &src, &scratch, Kind::Pass, &[], run, &mut r);
        let _ = std::fs::remove_dir_all(&base);
        r
    }

    #[test]
    #[cfg(unix)]
    fn allow_retries_reruns_the_binary() {
        // Fails on the first two runs (counting in the scratch dir, the
        // cwd), passes on the third.
        let flaky = "#!/bin/sh\necho x >> runs; [ $(wc -l < runs) -ge 3 ]\n";
        let secs = Duration::from_secs(20);
        let r = fake_run(flaky, RunLimits { timeout: secs, retries: 2 });
        assert_eq!(r.outcome, Outcome::Pass);
        assert_eq!(r.detail, "passed on attempt 3 of 3");
        let r = fake_run(flaky, RunLimits { timeout: secs, retries: 1 });
        assert_eq!(r.outcome, Outcome::Fail(Stage::Run));
        // A timeout is retried too, and the limit is the one given.
        let slow = "#!/bin/sh\nsleep 5\n";
        let r = fake_run(slow, RunLimits { timeout: Duration::from_millis(300), retries: 1 });
        assert_eq!(r.outcome, Outcome::Timeout(Stage::Run));
        assert!(r.wall < Duration::from_secs(4), "{:?}", r.wall);
    }

    #[test]
    fn mount_lookup() {
        let info = "\
531 152 0:68 / / rw,relatime - overlay overlay rw
564 531 0:47 /AgentWork/k /Volumes/AgentWork/k rw,relatime - fakeowner /run/host_mark/Volumes rw
565 531 0:48 / /a\\040b rw - tmpfs tmpfs rw
";
        let fs = |p: &str| mount_fs_type_in(info, Path::new(p));
        assert_eq!(fs("/Volumes/AgentWork/k/.build/x").as_deref(), Some("fakeowner"));
        assert_eq!(fs("/Volumes/AgentWork/kx").as_deref(), Some("overlay"));
        assert_eq!(fs("/a b/c").as_deref(), Some("tmpfs"));
        assert_eq!(fs("/tmp").as_deref(), Some("overlay"));
    }

    #[cfg(unix)]
    #[test]
    fn judging() {
        use std::os::unix::process::ExitStatusExt;
        use std::process::ExitStatus;
        let m = |raw: i32, stderr: &str| Measured {
            status: Some(ExitStatus::from_raw(raw)),
            stderr: stderr.as_bytes().to_vec(),
            wall: Duration::ZERO,
            max_rss_kb: None,
        };
        let keys = |s: Step| match s {
            Step::Failed { keys, crashed, .. } => {
                (keys.into_iter().map(|k| k.0).collect::<Vec<_>>(), crashed)
            }
            Step::Ok => (vec!["ok".into()], false),
            Step::Killed(_) => (vec!["killed".into()], false),
            Step::Timeout => (vec!["timeout".into()], false),
        };
        assert_eq!(keys(judge(Stage::Compile, &m(0, ""), false)).0, ["ok"]);
        assert_eq!(
            keys(judge(
                Stage::Compile,
                &m(256, "a.h:1:2: error: 'asm' statements is not supported yet\n"),
                false
            )),
            (vec!["compile: '…' statements is not supported yet".to_owned()], false)
        );
        let crash = "t.h:1:1: error: expected '>'\nthread 'kiln' panicked at crates/kiln_ast/src/decl.rs:1907:5:\ndecl has no Function payload\n";
        assert_eq!(
            keys(judge(Stage::Compile, &m(101 << 8, crash), false)),
            (
                vec![
                    "crash: panicked at crates/kiln_ast/src/decl.rs: decl has no Function payload"
                        .to_owned(),
                    "compile: expected '…'".to_owned()
                ],
                true
            )
        );
        assert_eq!(
            keys(judge(Stage::Compile, &m(11, ""), false)),
            (vec!["crash: killed by signal 11".to_owned()], true)
        );
        assert_eq!(
            keys(judge(Stage::Link, &m(256, "/usr/bin/ld: t.o: in function `main':\nt.cpp:(.text+0x1): undefined reference to `foo()'\n"), false)).0,
            ["link: undefined reference to '…'"]
        );
        assert_eq!(
            keys(judge(
                Stage::Run,
                &m(6, "t.exe: t.cpp:3: int main(): Assertion `x == 1' failed.\n"),
                false
            )),
            (vec!["run: assertion failed".to_owned()], false)
        );
        assert_eq!(keys(judge(Stage::Run, &m(256, ""), false)).0, ["run: exit code 1"]);
        assert_eq!(
            keys(judge(Stage::Compile, &m(134, "memory allocation of 8 bytes failed\n"), false)).0,
            ["killed"]
        );
        let timed_out =
            Measured { status: None, stderr: Vec::new(), wall: Duration::ZERO, max_rss_kb: None };
        assert_eq!(keys(judge(Stage::Run, &timed_out, false)).0, ["timeout"]);
        // spec/libcxx.md ".verify.cpp": a verify report clusters by entry;
        // other compile errors (a fatal include error) keep their keys.
        let report = "error: 'expected-error' diagnostics expected but not seen: \n  File t.cpp Line 3: deleted\n1 error generated.\n";
        assert_eq!(
            keys(judge(Stage::Compile, &m(256, report), true)).0,
            ["verify: error expected but not seen: deleted"]
        );
        assert_eq!(
            keys(judge(Stage::Compile, &m(256, "t.cpp:1:1: error: boom\n"), true)).0,
            ["compile: boom"]
        );
    }

    #[test]
    fn tails() {
        assert_eq!(tail("a\n\nb\nc\n", 2), "b\nc");
        assert_eq!(tail("", 3), "");
    }
}
