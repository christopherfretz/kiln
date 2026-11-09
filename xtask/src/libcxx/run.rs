//! Running one libc++ test with kiln (`spec/libcxx.md` "Commands" and
//! "Outcomes").

use std::fmt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use super::cluster;
use super::directives::{Directives, Kind, LocalCfg};
use super::features;
use super::script::{self, LineKind};
use crate::util::process::{self, Measured, MemLimit, RssProbe};

/// Everything a test run needs that does not vary per test.
pub struct Config {
    /// The compiler: kiln++, or clang in clang mode ([`Config::clang`]).
    pub kiln: PathBuf,
    /// Clang mode (spec/libcxx.md "Clang mode"): `--compiler` is a clang,
    /// run as the harness self-check. Its base flags have no
    /// `-resource-dir` and no `-fclang-compat`, and the libc++ headers are
    /// pinned to the tag's with `-nostdinc++ -isystem`.
    pub clang: bool,
    /// The libc++ headers of `llvm_dir` (`Oracle::libcxx_include`), pinned
    /// in clang mode when there is no [`LibcxxTree`].
    pub libcxx_include: PathBuf,
    /// The `llvm-project` checkout (holding `libcxx/test`).
    pub checkout: PathBuf,
    /// The release tag the checkout should be at (`--tag`).
    pub tag: String,
    /// `/usr/lib/llvm-N` of `KILN_ORACLE`: kiln++ runs with it as
    /// `KILN_LLVM_DIR` (`spec/harness.md` "Oracle toolchain").
    pub llvm_dir: PathBuf,
    /// kiln++'s `-fclang-compat`: the major of `llvm_dir`, so kiln
    /// identifies as the clang whose libc++ it compiles. Always passed
    /// (kiln++'s default is 22 since WP-394).
    pub clang_compat: u32,
    /// The libc++ to use when it is not `llvm_dir`'s own.
    pub libcxx: Option<LibcxxTree>,
    pub resource_dir: PathBuf,
    pub features: Vec<String>,
    /// `_LIBCPP_VERSION` of the libc++ headers (`features::libcxx_version`).
    pub libcxx_version: Option<u32>,
    pub compile_timeout: Duration,
    pub run_timeout: Duration,
    /// The run limit of a long test ([`is_long`]): `--run-timeout-long`.
    pub long_run_timeout: Duration,
    pub mem: Option<MemLimit>,
    pub probe: RssProbe,
    /// The flags of [`WARNING_FLAGS`] the compiler accepts
    /// ([`supported_warning_flags`]), passed to every compile with
    /// `-D_LIBCPP_HAS_NO_PRAGMA_SYSTEM_HEADER` (lit's `enable_warnings`).
    pub warning_flags: Vec<String>,
    /// The per-run scratch root; test `i` uses `<root>/t<i>` ([`scratch_name`]).
    pub scratch: PathBuf,
    /// The per-run `TMPDIR` root, in the system temp directory as lit's
    /// `lit-tmp-*`: test `i` gets `<tmp>/t<i>`, short enough for the paths
    /// a test builds under it (spec/libcxx.md "Commands").
    pub tmp: PathBuf,
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
        let mut v: Vec<String> = vec![format!("-std={}", features::STD), "-stdlib=libc++".into()];
        if !self.clang {
            v.extend(["-resource-dir".to_owned(), self.resource_dir.display().to_string()]);
        }
        v.extend(
            ["-pthread", "-ferror-limit=0", "-D_LIBCPP_ENABLE_EXPERIMENTAL"].map(str::to_owned),
        );
        if !self.clang {
            v.push(format!("-fclang-compat={}", self.clang_compat));
            // A crash is classified from the banner; a reproducer per
            // crashing test would only fill $TMPDIR (spec/libcxx.md).
            v.push("-fno-crash-diagnostics".to_owned());
        }
        // A clang finds the system's libc++ headers, which need not be the
        // tag's (clang++-22 here finds libc++ 19's): pin them.
        let include = match &self.libcxx {
            Some(t) => Some(&t.include),
            None => self.clang.then_some(&self.libcxx_include),
        };
        if let Some(include) = include {
            v.extend(["-nostdinc++".to_owned(), "-isystem".to_owned()]);
            v.push(include.display().to_string());
        }
        v.push("-I".to_owned());
        v.push(self.test_root().join("support").display().to_string());
        // lit's `enable_warnings` (default True, `params.py` lines 291-300):
        // the supported `_warningFlags`, and the libc++ headers are not
        // system headers (`.verify.cpp` undefines it again, [`VERIFY_FLAGS`]).
        if !self.warning_flags.is_empty() {
            v.extend(self.warning_flags.iter().cloned());
            v.push("-D_LIBCPP_HAS_NO_PRAGMA_SYSTEM_HEADER".to_owned());
        }
        v
    }

    /// The flags a test with `MODULE_DEPENDENCIES: std` gets: kiln finds
    /// `std.cppm` next to the libc++ headers `-stdlib=libc++` selects
    /// (spec/modules.md §2), which under `-nostdinc++ -isystem` are not the
    /// package tree's, so its `<prefix>/share/libc++/v1` is named with
    /// `-fprebuilt-module-path` (looked up before the default). Nothing in
    /// clang mode (clang wants a BMI there) or without a package tree.
    pub fn module_source_flags(&self) -> Vec<String> {
        if self.clang {
            return Vec::new();
        }
        let Some(t) = &self.libcxx else { return Vec::new() };
        // `<prefix>/include/c++/v1` -> `<prefix>/share/libc++/v1`.
        let Some(prefix) = t.include.parent().and_then(Path::parent).and_then(Path::parent) else {
            return Vec::new();
        };
        let share = prefix.join("share").join("libc++").join("v1");
        if !share.join("std.cppm").is_file() {
            return Vec::new();
        }
        vec![format!("-fprebuilt-module-path={}", share.display())]
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

/// libc++ 22's `_warningFlags`, verbatim and in order
/// (`libcxx/utils/libcxx/test/params.py` lines 17-81 at `llvmorg-22.1.8`).
/// lit adds each one the compiler accepts with `-Werror` to
/// `%{compile_flags}` (`dsl.py` `AddOptionalWarningFlag`), before the
/// test's `ADDITIONAL_COMPILE_FLAGS` (spec/libcxx.md "Commands").
pub const WARNING_FLAGS: &[&str] = &[
    "-Werror",
    "-Wall",
    "-Wctad-maybe-unsupported",
    "-Wextra",
    "-Wshadow",
    "-Wundef",
    "-Wunused-template",
    "-Wno-unused-command-line-argument",
    "-Wno-attributes",
    "-Wno-pessimizing-move",
    "-Wno-noexcept-type",
    "-Wno-aligned-allocation-unavailable",
    "-Wno-atomic-alignment",
    "-Wno-reserved-module-identifier",
    "-Wdeprecated-copy",
    "-Wdeprecated-copy-dtor",
    "-Wshift-negative-value",
    "-Wno-sized-deallocation",
    "-Wno-literal-suffix",
    "-Wno-user-defined-literals",
    "-Wno-tautological-compare",
    "-Wno-stringop-overread",
    "-Wno-stringop-overflow",
    "-Wsign-compare",
    "-Wunused-variable",
    "-Wunused-parameter",
    "-Wunreachable-code",
    "-Wno-unused-local-typedef",
    "-Wno-local-type-template-args",
    "-Wno-c++11-extensions",
    "-Wno-unknown-pragmas",
    "-Wno-pass-failed",
    "-Wno-dangling-reference",
    "-Wno-mismatched-new-delete",
    "-Wno-redundant-move",
    "-Wno-self-move",
    "-Wno-nullability-completeness",
    "-flax-vector-conversions=none",
];

/// The flags of [`WARNING_FLAGS`] that `compiler` accepts: lit's
/// `hasCompileFlag(config, "-Werror " + flag)`, an empty translation unit
/// compiled with `-Werror <flag>` (an unknown `-W` option is then an
/// error). Probed once per run, in `dir`.
pub fn supported_warning_flags(
    compiler: &Path,
    llvm_dir: &Path,
    dir: &Path,
    mem: Option<MemLimit>,
    timeout: Duration,
) -> Vec<String> {
    let _ = std::fs::create_dir_all(dir);
    let src = dir.join("w.cpp");
    if std::fs::write(&src, "").is_err() {
        return Vec::new();
    }
    let ok = |flag: &str| {
        let mut cmd = Command::new(compiler);
        cmd.args(["-xc++", "-fsyntax-only", "-Werror", flag])
            .arg(&src)
            .env("TMPDIR", dir)
            .env("KILN_LLVM_DIR", llvm_dir);
        process::run(&mut process::limited(&cmd, mem), timeout).is_ok_and(|o| o.success())
    };
    let v = WARNING_FLAGS.iter().filter(|f| ok(f)).map(|&f| f.to_owned()).collect();
    let _ = std::fs::remove_file(&src);
    let _ = std::fs::remove_dir(dir);
    v
}

/// The flags of a `.verify.cpp` compile: libc++'s `%{verify}`
/// substitution (`libcxx/utils/libcxx/test/format.py` line 102 at
/// `llvmorg-22.1.8`: the libc++ headers are system headers again,
/// `-Wno-error`), outcome from the exit status (spec/libcxx.md
/// ".verify.cpp").
pub const VERIFY_FLAGS: &[&str] = &[
    "-U_LIBCPP_HAS_NO_PRAGMA_SYSTEM_HEADER",
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
    /// The peak RSS of its kiln++ compile step, KiB.
    pub compile_rss_kb: Option<u64>,
    /// A `.compile.fail.cpp` test that passed on its exit status: the first
    /// `error:` line of the compile (with its location), or `(no error
    /// line)`, so an audit can spot a wrong-reason failure (spec/libcxx.md
    /// ".compile.fail.cpp").
    pub first_error: Option<String>,
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

/// Run as root, `setrlimit(RLIMIT_NPROC, 0)` does not stop `std::async`
/// from creating a thread (the kernel exempts root from the limit), so the
/// expected `resource_unavailable_try_again` never comes.
const ROOT_NPROC_UNSUPPORTED: &[&str] =
    &["std/thread/futures/futures.async/thread_create_failure.pass.cpp"];

/// The UNSUPPORTED reason of a [`ROOT_NPROC_UNSUPPORTED`] test run as root.
pub const ROOT_NPROC_REASON: &str = "env: running as root: RLIMIT_NPROC does not limit root";

/// Tests that need more address space than a `--mem-limit` below the
/// given MiB allows (the run step runs under the cap): the path, the MiB
/// it needs, and why. With the cap at or above that, or off (`0`), the
/// test runs. Sorted.
const MEM_UNSUPPORTED: &[(&str, u64, &str)] = &[(
    "std/input.output/string.streams/stringstream/stringstream.members/gcount.pass.cpp",
    7168,
    "an INT_MAX - 1 byte payload plus a stringbuf regrown past INT_MAX (2 + 1.5 + 3 GiB at the last regrowth, 6660 MiB measured)",
)];

/// With the scratch directory (`TMPDIR`) on a host-shared filesystem (a
/// Docker Desktop bind mount): a directory's hard-link count is not
/// `2 + subdirectories`, and `create_directory(p, attrs)` fails copying
/// the attributes.
const HOST_FS_UNSUPPORTED: &[&str] = &[
    "std/input.output/filesystems/fs.op.funcs/fs.op.create_directory/create_directory_with_attributes.pass.cpp",
    "std/input.output/filesystems/fs.op.funcs/fs.op.hard_lk_ct/hard_link_count.pass.cpp",
];

/// With the system tzdata at 2024b or later, `tzdata.zi` spells numeric
/// abbreviations as `%z`, which libc++ 19's tzdb parser keeps verbatim
/// (`sys_info::abbrev` is `"%z"` where `zdump` prints `"+03"`). libc++ 20
/// parses `%z` ([`TZDATA_Z_FIXED_IN`]), so the exclusion applies only below
/// that release.
const TZDATA_Z_UNSUPPORTED: &[&str] =
    &["std/time/time.zone/time.zone.timezone/time.zone.members/sys_info.zdump.pass.cpp"];

/// The first libc++ major release whose tzdb parser expands `%z`.
const TZDATA_Z_FIXED_IN: u32 = 20;

/// The system tzdata database `TZDATA_Z_UNSUPPORTED` depends on.
const TZDATA_ZI: &str = "/usr/share/zoneinfo/tzdata.zi";

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

/// Why `rel` is UNSUPPORTED on this host, if it is; `tmp` is where its
/// `TMPDIR` lives, `libcxx` the `_LIBCPP_VERSION` of the headers the run
/// uses (`features::libcxx_version`; unknown counts as affected), `mem` the
/// `--mem-limit` cap (`None`: off).
pub fn env_unsupported(
    rel: &str,
    tmp: &Path,
    libcxx: Option<u32>,
    mem: Option<MemLimit>,
) -> Option<String> {
    if ROOT_UNSUPPORTED.contains(&rel) && running_as_root() {
        return Some(ROOT_REASON.to_owned());
    }
    if ROOT_NPROC_UNSUPPORTED.contains(&rel) && running_as_root() {
        return Some(ROOT_NPROC_REASON.to_owned());
    }
    if let Some(&(_, need, why)) = MEM_UNSUPPORTED.iter().find(|(p, _, _)| *p == rel)
        && let Some(m) = mem.filter(|m| m.mib < need)
    {
        return Some(format!(
            "env: --mem-limit {} MiB is below the {need} MiB it needs: {why}",
            m.mib
        ));
    }
    if GROUPING_UNSUPPORTED.contains(&rel) && en_us_grouping_is_3() {
        return Some(GROUPING_REASON.to_owned());
    }
    if HOST_FS_UNSUPPORTED.contains(&rel)
        && let Some(fs) = mount_fs_type(tmp).filter(|t| HOST_FS_TYPES.contains(&t.as_str()))
    {
        return Some(format!("env: TMPDIR is on a host-shared {fs} mount"));
    }
    if TZDATA_Z_UNSUPPORTED.contains(&rel)
        && libcxx.is_none_or(|v| features::libcxx_major(v) < TZDATA_Z_FIXED_IN)
        && let Some(v) = tzdata_version(TZDATA_ZI).filter(|v| tzdata_uses_percent_z(v))
    {
        return Some(format!("env: tzdata {v} uses %z, which libc++ < 20 does not parse"));
    }
    None
}

/// The `# version YYYYx` of a `tzdata.zi` file, if readable.
fn tzdata_version(path: &str) -> Option<String> {
    let text = std::fs::read_to_string(path).ok()?;
    let v = text.lines().next()?.strip_prefix("# version ")?.trim();
    Some(v.to_owned())
}

/// Whether tzdata release `v` (`YYYYx`) is 2024b or later, the first to
/// use `%z` in `tzdata.zi`.
fn tzdata_uses_percent_z(v: &str) -> bool {
    v.len() == 5 && v.is_ascii() && v >= "2024b"
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

/// Tests whose `XFAIL:` the driver ignores, with the reason (spec/libcxx.md
/// "Outcomes"): they expect a failure that only lit's flags produce.
/// Sorted by path.
const XFAIL_IGNORED: &[(&str, &str)] = &[
    // None since WP-418: the driver applies lit's `-Werror` ([`WARNING_FLAGS`]),
    // so `libcxx/selftest/*/werror.*` fail as lit expects.
];

/// Why the driver ignores `rel`'s `XFAIL:`, if it does ([`XFAIL_IGNORED`]).
pub fn xfail_ignored(rel: &str) -> Option<&'static str> {
    XFAIL_IGNORED.iter().find(|&&(p, _)| p == rel).map(|&(_, r)| r)
}

/// Tests that pin a clang defect kiln does not share: XFAIL under kiln when
/// the headers are libc++ [`CLANG_DEFECT_SINCE`] or later, never in clang
/// mode (spec/libcxx.md "Outcomes"). Each one's `TEST_COMPILER_CLANG`
/// branch asserts clang's wrong answer, and kiln defines `__clang__`.
/// Sorted by path.
const CLANG_DEFECT_XFAIL: &[(&str, &str)] = &[
    (
        "std/concepts/concepts.compare/concept.equalitycomparable/equality_comparable_with.compile.pass.cpp",
        "pins llvm.org/PR171438 (a non-movable type is equality_comparable_with int under clang); kiln follows [concept.equalitycomparable]",
    ),
    (
        "std/language.support/cmp/cmp.concept/three_way_comparable_with.compile.pass.cpp",
        "pins llvm.org/PR171438 (a non-movable type is three_way_comparable_with int under clang); kiln follows [cmp.concept]",
    ),
];

/// The first libc++ major release whose tests pin the [`CLANG_DEFECT_XFAIL`]
/// behaviour.
const CLANG_DEFECT_SINCE: u32 = 22;

/// Why `rel` is XFAIL under kiln although lit expects it to pass, if it is
/// ([`CLANG_DEFECT_XFAIL`]); `libcxx` is the headers' `_LIBCPP_VERSION`.
pub fn clang_defect_xfail(rel: &str, clang: bool, libcxx: Option<u32>) -> Option<&'static str> {
    if clang || libcxx.is_none_or(|v| features::libcxx_major(v) < CLANG_DEFECT_SINCE) {
        return None;
    }
    CLANG_DEFECT_XFAIL.iter().find(|&&(p, _)| p == rel).map(|&(_, r)| r)
}

/// The M5 triage ledger (spec/libcxx.md "Outcomes", docs/status/libcxx-m5.md
/// "Triaged (b)"): `.verify` tests that fail under kiln only on a
/// diagnostic clang emits and kiln does not (or words differently), with
/// the diagnostic and the BACKLOG row that tracks it. XFAIL under kiln with
/// the libc++ [`M5_TRIAGED_LIBCXX`] headers, never in clang mode; a test
/// that starts passing is XPASS and leaves the list. Sorted by path.
const M5_TRIAGED: &[(&str, &str)] = &[
    (
        "libcxx/algorithms/cpp17_iterator_concepts.verify.cpp",
        "constraint note prints the substituted 'is_signed_v<unsigned int>' where clang prints the written 'is_signed_v<__iterator_difference_type<…>>'; no WP yet",
    ),
    (
        "libcxx/algorithms/specialized.algorithms/nonnull.verify.cpp",
        "missing -Wnonnull 'null passed to a callee that requires a non-null argument'; WP-433",
    ),
    (
        "libcxx/depr/depr.func.adaptor.typedefs/typedefs.depr_in_cxx17.verify.cpp",
        "missing \"'…' is deprecated\" on the C++17-deprecated adaptor typedefs (argument_type, result_type, …); no WP yet",
    ),
    (
        "libcxx/depr/depr.rel_ops/rel_ops.depr_in_cxx20.verify.cpp",
        "missing \"'rel_ops' is deprecated\" on a use of the deprecated namespace; no WP yet",
    ),
    (
        "libcxx/input.output/iostream.format/print.fun/nonnull.verify.cpp",
        "missing -Wnonnull 'null passed to a callee that requires a non-null argument'; WP-433",
    ),
    (
        "libcxx/mem/mem.res/nonnull.verify.cpp",
        "missing -Wnonnull 'null passed to a callee that requires a non-null argument'; WP-433",
    ),
    (
        "libcxx/strings/basic.string/nonnull.verify.cpp",
        "missing -Wnonnull 'null passed to a callee that requires a non-null argument'; WP-433",
    ),
    (
        "libcxx/strings/string.view/nonnull.verify.cpp",
        "missing -Wnonnull 'null passed to a callee that requires a non-null argument'; WP-433",
    ),
    (
        "libcxx/thread/thread.shared_mutex/thread_safety.verify.cpp",
        "missing -Wthread-safety 'writing variable … requires holding shared_mutex … exclusively'; no WP yet",
    ),
    (
        "libcxx/thread/thread.shared_timed_mutex/thread_safety.verify.cpp",
        "missing -Wthread-safety 'writing variable … requires holding shared_timed_mutex … exclusively'; no WP yet",
    ),
    (
        "libcxx/utilities/utility/mem.res/mem.poly.allocator.class/mem.poly.allocator.mem/allocate_vocabulary.attributes.verify.cpp",
        "missing alloc_align 'requested alignment is not a power of 2' on a constant argument; no WP yet",
    ),
    (
        "std/utilities/variant/variant.hash/hash.depr.verify.cpp",
        "prints 'std::hash<std::monostate>' where clang prints 'std::hash<monostate>' (template-argument type printing); WP-432",
    ),
];

/// The libc++ major release [`M5_TRIAGED`] was triaged against.
const M5_TRIAGED_LIBCXX: u32 = 22;

/// Why `rel` is XFAIL under kiln on the M5 ledger, if it is ([`M5_TRIAGED`]).
pub fn m5_triaged(rel: &str, clang: bool, libcxx: Option<u32>) -> Option<&'static str> {
    if clang || libcxx.is_none_or(|v| features::libcxx_major(v) != M5_TRIAGED_LIBCXX) {
        return None;
    }
    M5_TRIAGED.iter().find(|&&(p, _)| p == rel).map(|&(_, r)| r)
}

/// The XFAIL that applies to `rel` given lit's (`lit`, from [`prepare`]):
/// lit's unless [`XFAIL_IGNORED`], else the kiln-side lists
/// ([`CLANG_DEFECT_XFAIL`], `kiln: …`; [`M5_TRIAGED`], `m5: …`).
pub fn effective_xfail(
    rel: &str,
    lit: Option<String>,
    clang: bool,
    libcxx: Option<u32>,
) -> Option<String> {
    lit.filter(|_| xfail_ignored(rel).is_none())
        .or_else(|| clang_defect_xfail(rel, clang, libcxx).map(|r| format!("kiln: {r}")))
        .or_else(|| m5_triaged(rel, clang, libcxx).map(|r| format!("m5: {r}")))
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
    // Features a `lit.local.cfg` adds apply below it (lit loads the
    // configurations outermost first).
    let mut features: Vec<&str> = features.to_vec();
    for cfg in local_cfgs {
        if let Some(r) = cfg.unsupported(&features) {
            return Err(Outcome::Unsupported(r));
        }
        features.extend(cfg.add_features.iter().map(String::as_str));
    }
    let features = features.as_slice();
    let d = Directives::parse(text).map_err(Outcome::Error)?;
    let xfail = d.verdict(features).map_err(Outcome::Unsupported)?;
    if let Some(r) = kind.unsupported() {
        return Err(Outcome::Unsupported(r.to_owned()));
    }
    if kind == Kind::Sh {
        if d.run.is_empty() {
            return Err(Outcome::Unsupported("driver: .sh test without RUN lines".to_owned()));
        }
        // `format.py` prefixes the commands with `--precompile` builds of
        // the module dependencies; the driver does not reproduce them.
        if !d.module_dependencies.is_empty() {
            return Err(Outcome::Unsupported(
                "driver: RUN lines with MODULE_DEPENDENCIES (format.py --precompile)".to_owned(),
            ));
        }
        let subst: Vec<_> =
            local_cfgs.iter().flat_map(|c| c.substitutions.iter().cloned()).collect();
        script::check(&d.run, &subst).map_err(Outcome::Unsupported)?;
    } else if d.has_run {
        return Err(Outcome::Unsupported("driver: RUN lines".to_owned()));
    }
    // kiln builds `std`/`std.compat` in-process from libc++'s module
    // sources (spec/modules.md §2): no flags needed. Other modules have
    // no source the driver could find.
    if d.module_dependencies.iter().any(|m| m != "std" && m != "std.compat") {
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
    Failed {
        keys: Vec<(String, String)>,
        crashed: bool,
        detail: String,
        /// The first `error:` line of stderr, trimmed.
        first_error: Option<String>,
    },
    Killed(String),
    Timeout,
}

/// The arguments (between the compiler and the file) of `job`'s compile
/// at `-fsyntax-only`: the base flags and the test's own, as the compile
/// step of [`run_test`] gets them, and `%{verify}` ([`VERIFY_FLAGS`]) for a
/// `.verify.cpp` test. `Err` says why the test is not compiled that way: it
/// is unsupported here (environment, features, driver), `XFAIL`, a `.sh`
/// test, or a `.compile.fail` test (which is meant not to compile).
pub fn syntax_args(cfg: &Config, job: &Job<'_>) -> Result<Vec<String>, String> {
    let name = job.path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    let kind = Kind::of(name).ok_or_else(|| format!("{}: not a libc++ test file name", job.rel))?;
    if let Some(reason) = env_unsupported(&job.rel, &cfg.tmp, cfg.libcxx_version, cfg.mem) {
        return Err(format!("UNSUPPORTED ({reason})"));
    }
    let text = std::fs::read(&job.path)
        .map(|b| String::from_utf8_lossy(&b).into_owned())
        .map_err(|e| format!("cannot read {}: {e}", job.path.display()))?;
    let kind = compile_fail_kind(kind, &text);
    let (_, flags, xfail) =
        prepare(kind, &text, &job.local_cfgs, &cfg.feature_refs()).map_err(|o| o.to_string())?;
    if let Some(x) = xfail.filter(|_| xfail_ignored(&job.rel).is_none()) {
        return Err(format!("XFAIL: {x}"));
    }
    let mode: &[&str] = match kind {
        Kind::Sh | Kind::CompileFail => {
            return Err(format!("a {} test has no -fsyntax-only compile", kind.label()));
        }
        Kind::Verify => VERIFY_FLAGS,
        _ => &["-fsyntax-only"],
    };
    let mut v = cfg.base_compile_flags();
    v.extend(flags);
    v.extend(mode.iter().map(|f| (*f).to_owned()));
    Ok(v)
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
        compile_rss_kb: None,
        first_error: None,
    };
    if let Some(reason) = env_unsupported(&job.rel, &cfg.tmp, cfg.libcxx_version, cfg.mem) {
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
    let (d, mut flags, xfail) = match prepare(kind, &text, &job.local_cfgs, &features) {
        Ok(p) => p,
        Err(o) => {
            r.outcome = o;
            return r;
        }
    };
    if !d.module_dependencies.is_empty() {
        flags.extend(cfg.module_source_flags());
    }
    let xfail = effective_xfail(&job.rel, xfail, cfg.clang, cfg.libcxx_version);
    // lit's `%T`; `RUN:` lines can see it.
    let scratch = cfg.scratch.join(scratch_name(job.index));
    for dir in [&scratch, &test_tmpdir(cfg, &scratch)] {
        if let Err(e) = std::fs::create_dir_all(dir) {
            r.outcome = Outcome::Error(format!("cannot create {}: {e}", dir.display()));
            return r;
        }
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
    if kind == Kind::Sh {
        let subst: Vec<(String, String)> = job
            .local_cfgs
            .iter()
            .flat_map(|c| c.substitutions.iter())
            .filter_map(|(n, v)| Some((n.clone(), v.clone()?)))
            .collect();
        execute_script(cfg, &job.path, &scratch, &d.run, &flags, &subst, run, &mut r);
    } else {
        execute(cfg, &job.path, &scratch, compile_fail_kind(kind, &text), &flags, run, &mut r);
    }
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

/// A `.compile.fail.cpp` test with clang-verify directives is run as a
/// `.verify.cpp` one: its diagnostics must match (spec/libcxx.md
/// ".compile.fail.cpp"). Every other test keeps its kind.
fn compile_fail_kind(kind: Kind, text: &str) -> Kind {
    if kind == Kind::CompileFail && super::directives::has_verify_directives(text) {
        Kind::Verify
    } else {
        kind
    }
}

/// The scratch directory name of test `index`, also the last component of
/// its `TMPDIR`. Kept short: libc++'s filesystem tests build paths of
/// `TMPDIR/<cwd name>-static_env.<hash>/...` and compare
/// `filesystem_error::what()`, whose last byte libc++ 22 drops when the
/// message is 256 bytes or longer (`fs.op.copy_file/copy_file.pass.cpp`:
/// two such paths in one message, so each must stay under 103 bytes).
pub fn scratch_name(index: usize) -> String {
    format!("t{index}")
}

/// The `TMPDIR` root of run `pid`/`nanos`: in the system temp directory,
/// as lit's per-run `lit-tmp-*`, not under the target directory, so the
/// paths a test builds in it stay short ([`scratch_name`]).
pub fn tmp_root(pid: u32, nanos: u32) -> PathBuf {
    std::env::temp_dir().join(format!("kiln-tmp-{pid}-{nanos}"))
}

/// The `TMPDIR` of the test whose scratch directory is `scratch`.
fn test_tmpdir(cfg: &Config, scratch: &Path) -> PathBuf {
    cfg.tmp.join(scratch.file_name().unwrap_or_default())
}

fn cleanup(cfg: &Config, scratch: &Path) {
    if !cfg.keep {
        let _ = std::fs::remove_dir_all(scratch);
        let _ = std::fs::remove_dir_all(test_tmpdir(cfg, scratch));
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
        crate::util::isolate_caches(&mut c);
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
            // lit does not pass `TZ` to tests: the local zone is the
            // system's `/etc/localtime` (spec/libcxx.md "Commands").
            run.current_dir(scratch).env_remove("TZ");
            vec![
                (Stage::Compile, compile(&["-c", "-o", &obj_s]), true),
                (Stage::Link, link(), true),
                (Stage::Run, run, true),
            ]
        }
    };
    for (stage, mut cmd, expect_success) in steps {
        cmd.current_dir(scratch).env("TMPDIR", test_tmpdir(cfg, scratch));
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
        if !record(stage, verdict, expect_success, timeout, r) {
            return;
        }
    }
}

/// Records a step's verdict in `r`; `true` when the next step runs.
fn record(
    stage: Stage,
    verdict: Step,
    expect_success: bool,
    timeout: Duration,
    r: &mut TestResult,
) -> bool {
    match verdict {
        Step::Ok if expect_success => true,
        Step::Ok => {
            r.outcome = Outcome::Fail(Stage::ExpectedFail);
            let what = if stage == Stage::Compile { "compiled" } else { "linked" };
            r.keys = vec![(format!("{}.fail: {what} successfully", stage.label()), String::new())];
            false
        }
        Step::Failed { crashed: false, first_error, .. } if !expect_success => {
            if stage == Stage::Compile {
                r.first_error = Some(first_error.unwrap_or_else(|| "(no error line)".to_owned()));
            }
            false
        }
        Step::Failed { keys, crashed, detail, .. } => {
            r.outcome = Outcome::Fail(stage);
            r.keys = keys;
            r.crashed = crashed;
            r.detail = detail;
            false
        }
        Step::Killed(detail) => {
            r.outcome = Outcome::Killed(stage);
            r.detail = detail;
            false
        }
        Step::Timeout => {
            r.outcome = Outcome::Timeout(stage);
            r.detail = format!("killed after {}s", timeout.as_secs());
            false
        }
    }
}

/// Runs a `.sh.cpp` test's `RUN:` commands in order (spec/libcxx.md "RUN
/// lines"); the first that fails decides the outcome. A compiler command
/// is limited like a compile step, anything else like the run step (and
/// retried under `ALLOW_RETRIES:`). Each command's stdout joins its
/// stderr, so a `diff` or `grep` mismatch shows in the detail.
#[allow(clippy::too_many_arguments)] // the test, its flags and limits, the result
fn execute_script(
    cfg: &Config,
    src: &Path,
    scratch: &Path,
    lines: &[String],
    flags: &[String],
    cfg_subst: &[(String, String)],
    run: RunLimits,
    r: &mut TestResult,
) {
    let mut compile_flags = cfg.base_compile_flags();
    compile_flags.extend(flags.iter().cloned());
    let subst = script::Substitutions::new(
        &cfg.kiln,
        &compile_flags,
        &cfg.link_flags(),
        cfg_subst,
        src,
        scratch,
    );
    for line in lines {
        let expanded = match subst.expand(line) {
            Ok(e) => e,
            Err(e) => {
                r.outcome = Outcome::Error(e);
                return;
            }
        };
        let (stage, kind) = match script::line_kind(line) {
            LineKind::Compile { verify: true } => (Stage::Compile, Kind::Verify),
            LineKind::Compile { verify: false } => (Stage::Compile, Kind::Sh),
            LineKind::Link => (Stage::Link, Kind::Sh),
            LineKind::Run => (Stage::Run, Kind::Sh),
        };
        let mut cmd = Command::new("bash");
        crate::util::isolate_caches(&mut cmd);
        cmd.arg("-c").arg(script::bash_program(&expanded));
        cmd.current_dir(scratch)
            .env("TMPDIR", test_tmpdir(cfg, scratch))
            .env("KILN_LLVM_DIR", &cfg.llvm_dir)
            .env_remove("TZ");
        let is_kiln = stage != Stage::Run;
        let limit = if is_kiln {
            let words: Vec<&str> = expanded.split_whitespace().collect();
            cfg.mem.map(|m| m.plus(process::kiln_stack_reservation_mib(&words)))
        } else {
            cfg.mem
        };
        let timeout = if is_kiln { cfg.compile_timeout } else { run.timeout };
        let attempts = if is_kiln { 1 } else { run.retries + 1 };
        let verdict =
            match attempt_step(cfg, &cmd, scratch, stage, kind, timeout, limit, attempts, r) {
                Ok(v) => v,
                Err(e) => {
                    r.outcome = Outcome::Error(format!("cannot run `{line}`: {e}"));
                    return;
                }
            };
        if !record(stage, verdict, true, timeout, r) {
            if r.detail.is_empty() {
                r.detail = format!("RUN: {line}");
            } else {
                r.detail = format!("RUN: {line}\n{}", r.detail);
            }
            return;
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
        if stage == Stage::Compile {
            r.compile_rss_kb = r.compile_rss_kb.max(m.max_rss_kb);
        }
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

/// A diagnostic line with the directories of its location dropped
/// (`/…/libcxx/test/std/x.cpp:3:5: error: e` is `x.cpp:3:5: error: e`), so
/// a report names no host path.
fn short_path(line: &str) -> String {
    let line = line.trim();
    let at = line.find(": error: ").or_else(|| line.find(": fatal error: ")).unwrap_or(0);
    match line[..at].rfind('/') {
        Some(slash) => line[slash + 1..].to_owned(),
        None => line.to_owned(),
    }
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
    let first_error = stderr.lines().find(|l| cluster::error_message(l).is_some()).map(short_path);
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
            return Step::Failed { keys, crashed: true, detail, first_error };
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
    Step::Failed { keys, crashed: false, detail, first_error }
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
        assert_eq!(p(Kind::Pass, "// MODULE_DEPENDENCIES: std std.compat"), Ok((vec![], None)));
        assert_eq!(
            p(Kind::Pass, "// MODULE_DEPENDENCIES: other"),
            Err(Outcome::Unsupported("driver: modules".into()))
        );
        assert_eq!(p(Kind::Pass, "// XFAIL: kiln"), Ok((vec![], Some("kiln".into()))));
        assert!(matches!(p(Kind::Pass, "// XFAIL: (a"), Err(Outcome::Error(_))));
        let cfg = LocalCfg {
            conditions: vec![("no-localization".into(), false), ("c++experimental".into(), true)],
            ..LocalCfg::default()
        };
        assert_eq!(prepare(Kind::Pass, "", &[&cfg], F).map(|_| ()), Ok(()));
        assert_eq!(
            prepare(Kind::Pass, "", &[&LocalCfg::opaque()], F).map(|_| ()),
            Err(Outcome::Unsupported("driver: lit.local.cfg".into()))
        );
    }

    /// Clang mode drops kiln's own flags and pins the tag's libc++ headers
    /// (spec/libcxx.md "Clang mode").
    #[test]
    fn clang_mode_flags() {
        let mut cfg = Config {
            kiln: "kiln++".into(),
            clang: false,
            libcxx_include: "/llvm/include/c++/v1".into(),
            checkout: "/co".into(),
            tag: String::new(),
            llvm_dir: "/llvm".into(),
            clang_compat: 22,
            libcxx: None,
            resource_dir: "/res".into(),
            features: Vec::new(),
            libcxx_version: None,
            compile_timeout: Duration::from_secs(1),
            run_timeout: Duration::from_secs(1),
            long_run_timeout: Duration::from_secs(1),
            mem: None,
            probe: RssProbe::Unavailable,
            warning_flags: Vec::new(),
            scratch: "/s".into(),
            tmp: "/tmp".into(),
            keep: false,
        };
        let kiln = cfg.base_compile_flags().join(" ");
        assert!(kiln.contains("-resource-dir /res") && kiln.contains("-fclang-compat=22"));
        assert!(!kiln.contains("-nostdinc++"));
        cfg.clang = true;
        let clang = cfg.base_compile_flags().join(" ");
        assert!(!clang.contains("-resource-dir") && !clang.contains("-fclang-compat"));
        assert!(clang.contains("-nostdinc++ -isystem /llvm/include/c++/v1"));
        cfg.libcxx = Some(LibcxxTree { include: "/tree/inc".into(), lib: "/tree/lib".into() });
        assert!(cfg.base_compile_flags().join(" ").contains("-isystem /tree/inc"));
        // A `MODULE_DEPENDENCIES: std` test is pointed at the package
        // tree's `std.cppm`, in kiln mode only and only when it exists.
        let base = std::env::temp_dir().join(format!("kiln-libcxx-modsrc-{}", std::process::id()));
        let share = base.join("share").join("libc++").join("v1");
        std::fs::create_dir_all(&share).unwrap();
        let include = base.join("include").join("c++").join("v1");
        cfg.libcxx = Some(LibcxxTree { include, lib: "/tree/lib".into() });
        assert_eq!(cfg.module_source_flags(), Vec::<String>::new());
        cfg.clang = false;
        assert_eq!(cfg.module_source_flags(), Vec::<String>::new());
        std::fs::write(share.join("std.cppm"), "").unwrap();
        assert_eq!(
            cfg.module_source_flags(),
            [format!("-fprebuilt-module-path={}", share.display())]
        );
        cfg.clang = true;
        assert_eq!(cfg.module_source_flags(), Vec::<String>::new());
        let _ = std::fs::remove_dir_all(&base);
    }

    /// A `lit.local.cfg` that adds a feature satisfies `REQUIRES:` below
    /// it; `.sh.cpp` tests run unless their `RUN:` lines need something
    /// the driver lacks (spec/libcxx.md "RUN lines").
    #[test]
    fn preparation_of_scripts() {
        let added = LocalCfg { add_features: vec!["extra".into()], ..LocalCfg::default() };
        assert!(prepare(Kind::Pass, "// REQUIRES: extra", &[&added], F).is_ok());
        assert!(prepare(Kind::Pass, "// REQUIRES: extra", &[], F).is_err());
        let python =
            LocalCfg { substitutions: vec![("%{python}".into(), None)], ..LocalCfg::default() };
        let sh = |text: &str| prepare(Kind::Sh, text, &[&python], F).map(|_| ());
        assert_eq!(sh("// RUN: %{build}\n// RUN: %{run}"), Ok(()));
        assert_eq!(
            sh("// RUN: %{python} %s"),
            Err(Outcome::Unsupported("lit.local.cfg: unresolved substitution %{python}".into()))
        );
        assert_eq!(
            sh("// RUN: %{gdb} %t.exe"),
            Err(Outcome::Unsupported("driver: substitution %{gdb}".into()))
        );
        assert_eq!(
            sh("// MODULE_DEPENDENCIES: std\n// RUN: %{build}"),
            Err(Outcome::Unsupported(
                "driver: RUN lines with MODULE_DEPENDENCIES (format.py --precompile)".into()
            ))
        );
        assert_eq!(sh(""), Err(Outcome::Unsupported("driver: .sh test without RUN lines".into())));
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
        for list in [
            ROOT_UNSUPPORTED,
            ROOT_NPROC_UNSUPPORTED,
            HOST_FS_UNSUPPORTED,
            TZDATA_Z_UNSUPPORTED,
            GROUPING_UNSUPPORTED,
        ] {
            let mut sorted = list.to_vec();
            sorted.sort_unstable();
            sorted.dedup();
            assert_eq!(sorted, list);
        }
        let mem_paths: Vec<&str> = MEM_UNSUPPORTED.iter().map(|&(p, _, _)| p).collect();
        let mut sorted = mem_paths.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted, mem_paths);
        let tmp = std::env::temp_dir();
        let v19 = Some(190_107);
        let cap = MemLimit::from_mib(3072);
        assert_eq!(
            env_unsupported("std/utilities/any/any.class/nothing.pass.cpp", &tmp, v19, cap),
            None
        );
        let listed = env_unsupported(ROOT_UNSUPPORTED[0], &tmp, v19, cap);
        assert_eq!(listed.is_some(), running_as_root());
        assert!(listed.is_none_or(|r| r == ROOT_REASON));
        let listed = env_unsupported(ROOT_NPROC_UNSUPPORTED[0], &tmp, v19, cap);
        assert_eq!(listed.is_some(), running_as_root());
        assert!(listed.is_none_or(|r| r == ROOT_NPROC_REASON));
        let listed = env_unsupported(GROUPING_UNSUPPORTED[0], &tmp, v19, cap);
        assert_eq!(listed.is_some(), en_us_grouping_is_3());
        assert!(listed.is_none_or(|r| r == GROUPING_REASON));
        // A memory-hungry test is UNSUPPORTED only below the cap it needs.
        let (big, need, _) = MEM_UNSUPPORTED[0];
        let below = env_unsupported(big, &tmp, v19, cap).unwrap();
        assert!(below.starts_with("env: --mem-limit 3072 MiB is below the "), "{below}");
        assert!(env_unsupported(big, &tmp, v19, MemLimit::from_mib(need - 1)).is_some());
        assert_eq!(env_unsupported(big, &tmp, v19, MemLimit::from_mib(need)), None);
        assert_eq!(env_unsupported(big, &tmp, v19, None), None);
        // The `%z` exclusion follows the host tzdata below libc++ 20 and
        // never applies from 20 on.
        let z = tzdata_version(TZDATA_ZI).is_some_and(|v| tzdata_uses_percent_z(&v));
        assert_eq!(env_unsupported(TZDATA_Z_UNSUPPORTED[0], &tmp, v19, cap).is_some(), z);
        assert_eq!(env_unsupported(TZDATA_Z_UNSUPPORTED[0], &tmp, None, cap).is_some(), z);
        assert_eq!(env_unsupported(TZDATA_Z_UNSUPPORTED[0], &tmp, Some(200_100), cap), None);
        assert_eq!(env_unsupported(TZDATA_Z_UNSUPPORTED[0], &tmp, Some(220_000), cap), None);
        assert!(tzdata_uses_percent_z("2024b") && tzdata_uses_percent_z("2026b"));
        assert!(!tzdata_uses_percent_z("2024a") && !tzdata_uses_percent_z("2019c"));
        assert!(!tzdata_uses_percent_z("garbage"));
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

    #[test]
    fn xfail_ignored_list() {
        let paths: Vec<&str> = XFAIL_IGNORED.iter().map(|&(p, _)| p).collect();
        let mut sorted = paths.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted, paths);
        for &(path, reason) in XFAIL_IGNORED {
            assert!(path.starts_with("std/") || path.starts_with("libcxx/"), "{path}");
            assert!(Kind::of(path.rsplit('/').next().unwrap()).is_some(), "{path}");
            assert!(!reason.is_empty(), "{path} has no reason");
        }
        assert_eq!(xfail_ignored("libcxx/selftest/pass.cpp/werror.pass.cpp"), None);
        assert_eq!(xfail_ignored("libcxx/selftest/pass.cpp/xfail.pass.cpp"), None);
    }

    #[test]
    fn clang_defect_xfail_list() {
        let paths: Vec<&str> = CLANG_DEFECT_XFAIL.iter().map(|&(p, _)| p).collect();
        let mut sorted = paths.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted, paths);
        let (rel, reason) = CLANG_DEFECT_XFAIL[0];
        assert_ne!(reason, "");
        assert_eq!(clang_defect_xfail(rel, false, Some(220_108)), Some(reason));
        assert_eq!(clang_defect_xfail(rel, true, Some(220_108)), None);
        assert_eq!(clang_defect_xfail(rel, false, Some(190_107)), None);
        assert_eq!(clang_defect_xfail(rel, false, None), None);
        assert_eq!(clang_defect_xfail("std/a/b.pass.cpp", false, Some(220_108)), None);
    }

    #[test]
    fn m5_triaged_list() {
        let paths: Vec<&str> = M5_TRIAGED.iter().map(|&(p, _)| p).collect();
        let mut sorted = paths.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted, paths);
        for &(path, reason) in M5_TRIAGED {
            assert!(path.ends_with(".verify.cpp"), "{path}");
            assert!(!reason.is_empty(), "{path} has no reason");
        }
        // No test is on two lists.
        let mut all: Vec<&str> = XFAIL_IGNORED
            .iter()
            .chain(CLANG_DEFECT_XFAIL)
            .chain(M5_TRIAGED)
            .map(|&(p, _)| p)
            .collect();
        let n = all.len();
        all.sort_unstable();
        all.dedup();
        assert_eq!(all.len(), n, "a path is on two XFAIL lists");
        // Every path names a test of the libc++ 22 checkout, when there is one.
        let root = crate::util::repo_root();
        let tests = super::super::default_checkout(&root, super::super::M4_TAG).join("libcxx/test");
        if tests.is_dir() {
            for &(path, _) in M5_TRIAGED {
                assert!(tests.join(path).is_file(), "{path} is not in {}", tests.display());
            }
        } else {
            eprintln!("m5_triaged_list: no {}; paths not checked", tests.display());
        }
        let (rel, reason) = M5_TRIAGED[0];
        assert_eq!(m5_triaged(rel, false, Some(220_108)), Some(reason));
        assert_eq!(m5_triaged(rel, true, Some(220_108)), None);
        assert_eq!(m5_triaged(rel, false, Some(190_107)), None);
        assert_eq!(m5_triaged(rel, false, Some(230_000)), None);
        assert_eq!(m5_triaged(rel, false, None), None);
        assert_eq!(effective_xfail(rel, None, false, Some(220_108)), Some(format!("m5: {reason}")));
        assert_eq!(effective_xfail(rel, None, true, Some(220_108)), None);
        let lit = Some("*".to_owned());
        assert_eq!(effective_xfail("std/a/b.pass.cpp", lit.clone(), false, None), lit);
        let (defect, why) = CLANG_DEFECT_XFAIL[0];
        assert_eq!(
            effective_xfail(defect, None, false, Some(220_108)),
            Some(format!("kiln: {why}"))
        );
    }

    #[test]
    fn short_scratch_paths() {
        assert_eq!(scratch_name(0), "t0");
        assert_eq!(scratch_name(12_345), "t12345");
        // The deepest path `fs.op.copy_file/copy_file.pass.cpp` puts in a
        // `filesystem_error` message: `<TMPDIR>/<cwd name>-static_env.<hash>
        // /file1`, with a 20-digit hash; two of them must keep the message
        // under 256 bytes ([`scratch_name`]), i.e. each under 103 bytes.
        let tmp = tmp_root(4_194_304, 999_999_999).join(scratch_name(99_999));
        let deepest = tmp.join(format!("{}-static_env.{}/file1", scratch_name(99_999), u64::MAX));
        // With the system temp directory `/tmp` (a longer `TMPDIR` costs
        // its extra bytes, as lit's `lit-tmp-*` does).
        let below = deepest.strip_prefix(std::env::temp_dir()).unwrap();
        let len = "/tmp/".len() + below.as_os_str().len();
        assert!(len < 103, "{} is {len} bytes under /tmp", below.display());
    }

    /// Writes an executable script at `path` without this process ever
    /// holding it open for writing. `cargo test` runs tests on threads, and
    /// a child forked by another thread inherits every open descriptor
    /// until its `exec`; executing a file such a child still holds open for
    /// writing fails with `ETXTBSY` ("Text file busy"), however promptly
    /// this thread closed its own handle. So the text goes to a side file
    /// that is never executed, and a child process (`cp`, then `chmod`)
    /// creates the executable itself.
    #[cfg(unix)]
    fn write_script(path: &Path, text: &str) {
        let side = path.with_extension("txt");
        std::fs::write(&side, text).unwrap();
        let ok = Command::new("sh")
            .args(["-c", "cp \"$1\" \"$2\" && chmod 755 \"$2\"", "sh"])
            .arg(&side)
            .arg(path)
            .status()
            .unwrap()
            .success();
        assert!(ok, "cannot create {}", path.display());
    }

    /// A fake compiler: `-c -o t.o` writes the object, the link step writes
    /// `t.exe` as a copy of the test source (a shell script), so the run
    /// step runs the test's own script.
    #[cfg(unix)]
    fn fake_run(script: &str, run: RunLimits) -> TestResult {
        fake_run_lines(script, &[], run)
    }

    /// `fake_run` of a `.sh.cpp` test with the `RUN:` commands `lines`
    /// (none: a `.pass.cpp` test).
    fn fake_run_lines(script: &str, lines: &[&str], run: RunLimits) -> TestResult {
        fake_exec(None, Kind::Pass, script, lines, run)
    }

    /// `fake_run_lines` with the fake compiler `kiln` (a shell script; the
    /// default one when `None`) and the test kind `kind`.
    fn fake_exec(
        kiln: Option<&str>,
        kind: Kind,
        script: &str,
        lines: &[&str],
        run: RunLimits,
    ) -> TestResult {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static SEQ: AtomicUsize = AtomicUsize::new(0);
        let base = std::env::temp_dir().join(format!(
            "kiln-libcxx-retry-{}-{}",
            std::process::id(),
            SEQ.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).unwrap();
        let kiln_script = kiln;
        let kiln = base.join("fake-kiln");
        let src = base.join("t.pass.cpp");
        let default = format!(
            "#!/bin/sh\nfor a; do case \"$a\" in -c) exec touch t.o;; esac; done\n\
             cp {} t.exe && chmod +x t.exe\n",
            src.display()
        );
        write_script(&kiln, kiln_script.unwrap_or(&default));
        std::fs::write(&src, script).unwrap();
        let cfg = Config {
            kiln,
            clang: false,
            libcxx_include: base.clone(),
            checkout: base.clone(),
            tag: String::new(),
            llvm_dir: base.clone(),
            clang_compat: 19,
            libcxx: None,
            resource_dir: base.clone(),
            features: Vec::new(),
            libcxx_version: None,
            compile_timeout: Duration::from_secs(20),
            run_timeout: run.timeout,
            long_run_timeout: run.timeout,
            mem: None,
            probe: RssProbe::Unavailable,
            warning_flags: Vec::new(),
            scratch: base.join("s"),
            tmp: base.join("tmp"),
            keep: false,
        };
        let scratch = base.join("s");
        std::fs::create_dir_all(&scratch).unwrap();
        std::fs::create_dir_all(base.join("tmp/s")).unwrap();
        let mut r = TestResult {
            rel: "t.pass.cpp".into(),
            outcome: Outcome::Pass,
            keys: Vec::new(),
            crashed: false,
            detail: String::new(),
            wall: Duration::ZERO,
            max_rss_kb: None,
            compile_rss_kb: None,
            first_error: None,
        };
        if lines.is_empty() {
            execute(&cfg, &src, &scratch, kind, &[], run, &mut r);
        } else {
            let lines: Vec<String> = lines.iter().map(|l| (*l).to_owned()).collect();
            let flags = ["-DEXTRA".to_owned()];
            execute_script(&cfg, &src, &scratch, &lines, &flags, &[], run, &mut r);
        }
        let _ = std::fs::remove_dir_all(&base);
        r
    }

    /// The libc++ slice's compiles (spec/bench.md "The libcxx-slice
    /// entry") are the runner's: base flags, the test's own flags, then
    /// `-fsyntax-only` or the verify flags; tests with no syntax-only
    /// compile are refused.
    #[test]
    fn syntax_args_follow_the_runner() {
        let base = std::env::temp_dir().join(format!("kiln-libcxx-syntax-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).unwrap();
        let cfg = Config {
            kiln: base.join("kiln++"),
            clang: false,
            libcxx_include: base.join("inc"),
            checkout: base.clone(),
            tag: String::new(),
            llvm_dir: base.clone(),
            clang_compat: 22,
            libcxx: None,
            resource_dir: base.join("res"),
            features: F.iter().map(|f| (*f).to_owned()).collect(),
            libcxx_version: None,
            compile_timeout: Duration::from_secs(20),
            run_timeout: Duration::from_secs(20),
            long_run_timeout: Duration::from_secs(20),
            mem: None,
            probe: RssProbe::Unavailable,
            warning_flags: vec!["-Wall".into()],
            scratch: base.join("s"),
            tmp: base.join("t"),
            keep: false,
        };
        let args = |name: &str, text: &str| {
            let path = base.join(name);
            std::fs::write(&path, text).unwrap();
            let job = Job { path, rel: name.to_owned(), local_cfgs: Vec::new(), index: 0 };
            syntax_args(&cfg, &job)
        };
        let base_flags = cfg.base_compile_flags();
        let pass =
            args("a.pass.cpp", "// ADDITIONAL_COMPILE_FLAGS: -DX=1\nint main(){}\n").unwrap();
        assert_eq!(&pass[..base_flags.len()], base_flags.as_slice());
        assert_eq!(&pass[base_flags.len()..], ["-DX=1", "-fsyntax-only"]);
        let verify = args("b.verify.cpp", "// expected-no-diagnostics\n").unwrap();
        let tail: Vec<&str> = verify[base_flags.len()..].iter().map(String::as_str).collect();
        assert_eq!(tail, VERIFY_FLAGS);
        let sh = args("c.sh.cpp", "// RUN: %{build}\n").unwrap_err();
        assert!(sh.contains("no -fsyntax-only compile"), "{sh}");
        let unsupported = args("d.pass.cpp", "// UNSUPPORTED: c++26\n").unwrap_err();
        assert!(unsupported.contains("UNSUPPORTED"), "{unsupported}");
        let xfail = args("e.pass.cpp", "// XFAIL: kiln\n").unwrap_err();
        assert!(xfail.starts_with("XFAIL"), "{xfail}");
        let _ = std::fs::remove_dir_all(&base);
    }

    /// libc++'s `.sh.cpp` idioms: `%{build}`, output redirected and
    /// `diff`ed, `echo … | grep …` over `%{compile_flags}`, lit's `not`
    /// (spec/libcxx.md "RUN lines").
    #[test]
    #[cfg(unix)]
    fn script_lines_run_in_order() {
        let limits = RunLimits { timeout: Duration::from_secs(20), retries: 0 };
        let mut lines = vec![
            "%{build}",
            "%{exec} %t.exe > %t.actual",
            "echo -n hi > %t.expected",
            "diff %t.expected %t.actual",
            "echo \"%{compile_flags}\" | grep -e -DEXTRA",
            "echo \"%{compile_flags}\" | not grep -e -DABSENT",
            "test -d %T",
        ];
        let r = fake_run_lines("#!/bin/sh\nprintf hi\n", &lines, limits);
        assert_eq!(r.outcome, Outcome::Pass, "{}", r.detail);
        lines[2] = "echo -n ho > %t.expected";
        let r = fake_run_lines("#!/bin/sh\nprintf hi\n", &lines, limits);
        assert_eq!(r.outcome, Outcome::Fail(Stage::Run));
        assert!(r.detail.starts_with("RUN: diff %t.expected %t.actual"), "{}", r.detail);
        let r = fake_run_lines("#!/bin/sh\nexit 0\n", &["false | true"], limits);
        assert_eq!(r.outcome, Outcome::Fail(Stage::Run), "pipefail");
    }

    /// A `.compile.fail.cpp` test without verify directives passes on a
    /// failed compile and records its first error (directories dropped); with
    /// directives it runs as a `.verify.cpp` test, so only a matching
    /// `-verify` (exit 0) passes (spec/libcxx.md ".compile.fail.cpp").
    #[test]
    #[cfg(unix)]
    fn compile_fail_records_first_error_or_verifies() {
        let limits = RunLimits { timeout: Duration::from_secs(20), retries: 0 };
        let kiln = "#!/bin/sh\ncase \"$*\" in *-verify*) exit 0;; esac\n\
                    echo '/a/b/t.cpp:3:5: warning: w' >&2\n\
                    echo '/a/b/t.cpp:4:1: error: first' >&2\n\
                    echo 't.cpp:9:1: error: second' >&2\nexit 1\n";
        let r = fake_exec(Some(kiln), Kind::CompileFail, "", &[], limits);
        assert_eq!(r.outcome, Outcome::Pass, "{}", r.detail);
        assert_eq!(r.first_error.as_deref(), Some("t.cpp:4:1: error: first"));
        let r = fake_exec(Some("#!/bin/sh\nexit 1\n"), Kind::CompileFail, "", &[], limits);
        assert_eq!(r.first_error.as_deref(), Some("(no error line)"));
        let r = fake_exec(Some("#!/bin/sh\nexit 0\n"), Kind::CompileFail, "", &[], limits);
        assert_eq!(r.outcome, Outcome::Fail(Stage::ExpectedFail));
        assert_eq!(r.first_error, None);
        // With directives: the verify run decides.
        let kind = compile_fail_kind(Kind::CompileFail, "f(); // expected-error {{x}}");
        assert_eq!(kind, Kind::Verify);
        assert_eq!(compile_fail_kind(Kind::CompileFail, "// fails"), Kind::CompileFail);
        assert_eq!(compile_fail_kind(Kind::Pass, "// expected-error {{x}}"), Kind::Pass);
        let r = fake_exec(Some(kiln), kind, "", &[], limits);
        assert_eq!((r.outcome, r.first_error), (Outcome::Pass, None));
        let r = fake_exec(Some("#!/bin/sh\nexit 1\n"), kind, "", &[], limits);
        assert_eq!(r.outcome, Outcome::Fail(Stage::Compile));
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

    /// Regression for the `ETXTBSY` flake: fake compilers are written and
    /// executed in a loop while another thread keeps forking children.
    #[test]
    #[cfg(unix)]
    fn fresh_scripts_execute_while_other_threads_fork() {
        use std::sync::atomic::{AtomicBool, Ordering};
        let stop = std::sync::Arc::new(AtomicBool::new(false));
        let forkers: Vec<_> = (0..4)
            .map(|_| {
                let stop = std::sync::Arc::clone(&stop);
                std::thread::spawn(move || {
                    while !stop.load(Ordering::Relaxed) {
                        let _ = Command::new("true").status();
                    }
                })
            })
            .collect();
        let secs = Duration::from_secs(20);
        for _ in 0..50 {
            let r = fake_run("#!/bin/sh\nexit 0\n", RunLimits { timeout: secs, retries: 0 });
            assert_eq!(r.outcome, Outcome::Pass, "{}", r.detail);
        }
        stop.store(true, Ordering::Relaxed);
        for f in forkers {
            f.join().unwrap();
        }
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
