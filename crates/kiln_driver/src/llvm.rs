//! Finding the LLVM installation kiln drives (spec/driver.md "LLVM
//! discovery"): the `opt`, `llc` and `llvm-as` that consume kiln's IR,
//! the `clang++` that links, and the libc++ headers beside them.
//!
//! The candidates, first valid one wins:
//!
//! 1. `--llvm-dir=<dir>`;
//! 2. `$KILN_LLVM_DIR` (non-empty);
//! 3. the build's `KILN_DEFAULT_LLVM_DIR` (a distribution package's own
//!    LLVM), then, for each supported major in preference order
//!    ([`SUPPORTED_LLVM_MAJORS`]: 22 first, then 19):
//!    1. an `llvm-config` on `PATH` whose installation has that major,
//!       then `llvm-config-<N>`; the installation is the directory above
//!       the one holding it, symlinks resolved (what `llvm-config
//!       --prefix` prints for an installed LLVM);
//!    2. the usual layouts: `/usr/lib/llvm-<N>` (Debian, Ubuntu,
//!       apt.llvm.org), `/usr/lib64/llvm<N>` (Fedora's compat packages),
//!       `/home/linuxbrew/.linuxbrew/opt/llvm@<N>` and
//!       `/home/linuxbrew/.linuxbrew/opt/llvm` (Homebrew on Linux), `/usr`
//!       (Fedora's main LLVM);
//!    3. an `opt-<N>` or `clang++-<N>` on `PATH`: the installation above
//!       its resolved location.
//!
//! A candidate is valid when it has `bin/opt`, `bin/llc` and `bin/clang++`
//! and its major ([`dir_major`]) is supported. An explicit choice (1 or 2)
//! that is not valid is an error naming it, never a fall-through; an
//! invalid implicit one is skipped, and when none is valid the error lists
//! what was tried ([`LlvmError::NotFound`]).

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use kiln_diag::{Diag, DiagEngine, DiagId};
use kiln_source::Span;

/// The LLVM majors kiln supports, in preference order: the implicit search
/// looks for the first, then the next. Supporting another major is one
/// entry here (plus its tests).
pub const SUPPORTED_LLVM_MAJORS: &[u32] = &[22, 19];

/// The preferred major: the IR dialect when no LLVM is found (`-S
/// -emit-llvm` needs no tool).
pub const DEFAULT_LLVM_MAJOR: u32 = SUPPORTED_LLVM_MAJORS[0];

/// The tools a valid installation has in `bin/`: those every compile or
/// link may run (`llvm-as` and `llvm-objcopy` ship with `llc`).
pub const REQUIRED_TOOLS: &[&str] = &["opt", "llc", "clang++"];

/// The LLVM installation used when nothing explicit is given, if kiln was
/// built with `KILN_DEFAULT_LLVM_DIR` set and non-empty (a distribution
/// package whose LLVM lives elsewhere): tried before the search.
pub const BUILD_DEFAULT_LLVM_DIR: Option<&str> = match option_env!("KILN_DEFAULT_LLVM_DIR") {
    Some(d) if !d.is_empty() => Some(d),
    _ => None,
};

/// Homebrew on Linux's prefix.
const LINUXBREW: &str = "/home/linuxbrew/.linuxbrew/opt";

/// `19 and 22`: the supported majors, ascending, for messages.
pub fn supported_text() -> String {
    let mut v = SUPPORTED_LLVM_MAJORS.to_vec();
    v.sort_unstable();
    let parts: Vec<String> = v.iter().map(u32::to_string).collect();
    match parts.split_last() {
        Some((last, rest)) if !rest.is_empty() => format!("{} and {last}", rest.join(", ")),
        _ => parts.join(""),
    }
}

/// Where an installation came from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Origin {
    /// `--llvm-dir=`.
    Flag,
    /// `KILN_LLVM_DIR`.
    Env,
    /// The build's `KILN_DEFAULT_LLVM_DIR`.
    BuildDefault,
    /// An `llvm-config` (the path found on `PATH`).
    LlvmConfig(PathBuf),
    /// One of the usual layouts.
    Layout,
    /// A versioned tool on `PATH` (its path).
    PathTool(PathBuf),
}

impl Origin {
    /// Whether the user named the installation (an invalid one is then an
    /// error).
    pub fn is_explicit(&self) -> bool {
        matches!(self, Origin::Flag | Origin::Env)
    }

    /// How diagnostics and `-v` name the origin.
    pub fn describe(&self) -> String {
        match self {
            Origin::Flag => "--llvm-dir".to_owned(),
            Origin::Env => "KILN_LLVM_DIR".to_owned(),
            Origin::BuildDefault => "the build default".to_owned(),
            Origin::LlvmConfig(p) | Origin::PathTool(p) => p.display().to_string(),
            Origin::Layout => "the default layouts".to_owned(),
        }
    }
}

/// A valid LLVM installation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LlvmInstall {
    /// Its prefix (`<dir>/bin/llc`).
    pub dir: PathBuf,
    /// Its major version.
    pub major: u32,
    /// How it was found.
    pub origin: Origin,
}

/// Why a candidate directory is not a valid installation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Reject {
    /// Not a directory.
    Missing,
    /// No `bin/<tool>`.
    NoTool(&'static str),
    /// Its major cannot be told (no `lib/clang/<N>`, no working
    /// `bin/llvm-config`, not named `llvm-<N>`).
    NoMajor,
    /// Its major is not supported.
    Unsupported(u32),
}

impl Reject {
    /// The reason, as diagnostics print it.
    pub fn describe(&self) -> String {
        match self {
            Reject::Missing => "no such directory".to_owned(),
            Reject::NoTool(t) => format!("no 'bin/{t}'"),
            Reject::NoMajor => {
                "cannot tell its version (no 'lib/clang/<N>' or 'bin/llvm-config')".to_owned()
            }
            Reject::Unsupported(m) => format!("LLVM {m} is not supported"),
        }
    }
}

/// Why no installation was selected.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LlvmError {
    /// The explicit choice is not an installation.
    Invalid {
        /// The directory named.
        dir: PathBuf,
        /// [`Origin::Flag`] or [`Origin::Env`].
        origin: Origin,
        /// Why (never [`Reject::Unsupported`]).
        reason: Reject,
    },
    /// The explicit choice has an unsupported major.
    Unsupported {
        /// The directory named.
        dir: PathBuf,
        /// [`Origin::Flag`] or [`Origin::Env`].
        origin: Origin,
        /// Its major.
        major: u32,
    },
    /// No implicit candidate is valid: each tried directory with why it was
    /// skipped, then the `PATH` names that were not found.
    NotFound {
        /// The candidate directories, in order.
        tried: Vec<(PathBuf, Reject)>,
        /// The `PATH` tools looked for and not found.
        not_on_path: Vec<String>,
    },
}

impl LlvmError {
    /// Whether the user's own choice failed (reported before anything
    /// runs, whatever the action).
    pub fn is_explicit(&self) -> bool {
        !matches!(self, LlvmError::NotFound { .. })
    }

    /// Reports the error on `drv`: [`LlvmError::Invalid`] and
    /// [`LlvmError::Unsupported`] as one error naming the directory and its
    /// origin, [`LlvmError::NotFound`] as an error with a note per skipped
    /// directory, one for the `PATH` names not found, and how to fix it.
    pub fn emit(&self, drv: &DiagEngine<'_>) {
        let d = match self {
            LlvmError::Invalid { dir, origin, reason } => {
                Diag::new(DiagId::drv_llvm_dir_invalid, Span::INVALID)
                    .arg(dir.display())
                    .arg(origin.describe())
                    .arg(reason.describe())
            }
            LlvmError::Unsupported { dir, origin, major } => {
                Diag::new(DiagId::drv_llvm_unsupported, Span::INVALID)
                    .arg(major)
                    .arg(dir.display())
                    .arg(origin.describe())
                    .arg(supported_text())
            }
            LlvmError::NotFound { tried, not_on_path } => {
                let mut d =
                    Diag::new(DiagId::drv_llvm_not_found, Span::INVALID).arg(supported_text());
                let note = |id| Diag::new(id, Span::INVALID);
                for (dir, why) in tried {
                    d = d.with_note(
                        note(DiagId::drv_llvm_tried).arg(dir.display()).arg(why.describe()),
                    );
                }
                if !not_on_path.is_empty() {
                    d = d.with_note(note(DiagId::drv_llvm_not_on_path).arg(not_on_path.join(", ")));
                }
                let prefer: Vec<String> =
                    SUPPORTED_LLVM_MAJORS.iter().map(u32::to_string).collect();
                d.with_note(note(DiagId::drv_llvm_install_hint).arg(prefer.join(" or ")))
            }
        };
        d.emit(drv);
    }
}

/// The selected installation, or why there is none.
pub type LlvmSelection = Result<LlvmInstall, LlvmError>;

/// The inputs of the search, separated from the process environment so the
/// search can be tested on fake trees.
#[derive(Clone, Debug, Default)]
pub struct Search {
    /// `--llvm-dir=`.
    pub flag: Option<String>,
    /// `$KILN_LLVM_DIR`, if set and non-empty.
    pub env: Option<String>,
    /// The build's `KILN_DEFAULT_LLVM_DIR`.
    pub build_default: Option<String>,
    /// `$PATH`.
    pub path: Option<OsString>,
    /// Prepended to the fixed layout paths (`""` in the driver; a scratch
    /// tree in tests).
    pub root: String,
}

impl Search {
    /// The driver's search: `flag` and the process environment.
    pub fn from_env(flag: Option<&str>) -> Search {
        Search {
            flag: flag.map(str::to_owned),
            env: std::env::var("KILN_LLVM_DIR").ok().filter(|d| !d.is_empty()),
            build_default: BUILD_DEFAULT_LLVM_DIR.map(str::to_owned),
            path: std::env::var_os("PATH"),
            root: String::new(),
        }
    }

    /// Runs the search.
    pub fn run(&self) -> LlvmSelection {
        let explicit = match (&self.flag, &self.env) {
            (Some(d), _) => Some((d, Origin::Flag)),
            (None, Some(d)) => Some((d, Origin::Env)),
            (None, None) => None,
        };
        if let Some((dir, origin)) = explicit {
            let dir = PathBuf::from(dir);
            return match check(&dir) {
                Ok(major) => Ok(LlvmInstall { dir, major, origin }),
                Err(Reject::Unsupported(major)) => {
                    Err(LlvmError::Unsupported { dir, origin, major })
                }
                Err(reason) => Err(LlvmError::Invalid { dir, origin, reason }),
            };
        }
        let mut tried: Vec<(PathBuf, Reject)> = Vec::new();
        let mut not_on_path: Vec<String> = Vec::new();
        let mut seen: Vec<PathBuf> = Vec::new();
        // Each directory is checked once; a second sighting (the same
        // prefix through `llvm-config` and its layout) is skipped.
        let mut try_dir = |dir: PathBuf, origin: Origin, want: Option<u32>| {
            if seen.contains(&dir) {
                return None;
            }
            seen.push(dir.clone());
            match check(&dir) {
                Ok(major) if want.is_none_or(|w| w == major) => {
                    Some(LlvmInstall { dir, major, origin })
                }
                // Another supported major: its own pass may take it.
                Ok(_) => {
                    seen.pop();
                    None
                }
                Err(r) => {
                    tried.push((dir, r));
                    None
                }
            }
        };
        if let Some(d) = &self.build_default
            && let Some(i) = try_dir(PathBuf::from(d), Origin::BuildDefault, None)
        {
            return Ok(i);
        }
        let mut path_lookup = |name: &str| {
            let found = find_on_path(self.path.as_ref(), name);
            if found.is_none() && !not_on_path.iter().any(|n| n == name) {
                not_on_path.push(name.to_owned());
            }
            found
        };
        for &m in SUPPORTED_LLVM_MAJORS {
            for name in ["llvm-config".to_owned(), format!("llvm-config-{m}")] {
                if let Some(tool) = path_lookup(&name)
                    && let Some(prefix) = prefix_of_tool(&tool)
                    && let Some(i) = try_dir(prefix, Origin::LlvmConfig(tool), Some(m))
                {
                    return Ok(i);
                }
            }
            for layout in [
                format!("/usr/lib/llvm-{m}"),
                format!("/usr/lib64/llvm{m}"),
                format!("{LINUXBREW}/llvm@{m}"),
                format!("{LINUXBREW}/llvm"),
                "/usr".to_owned(),
            ] {
                let dir = PathBuf::from(format!("{}{layout}", self.root));
                if let Some(i) = try_dir(dir, Origin::Layout, Some(m)) {
                    return Ok(i);
                }
            }
            for name in [format!("opt-{m}"), format!("clang++-{m}")] {
                if let Some(tool) = path_lookup(&name)
                    && let Some(prefix) = prefix_of_tool(&tool)
                    && let Some(i) = try_dir(prefix, Origin::PathTool(tool), Some(m))
                {
                    return Ok(i);
                }
            }
        }
        // A directory of the wrong major for every pass was popped; the
        // rest is what was tried.
        Err(LlvmError::NotFound { tried, not_on_path })
    }
}

/// The first `PATH` entry holding the file `name`.
fn find_on_path(path: Option<&OsString>, name: &str) -> Option<PathBuf> {
    std::env::split_paths(path?).map(|d| d.join(name)).find(|p| p.is_file())
}

/// The installation prefix of a tool found on `PATH`: the directory above
/// the one holding it, after resolving symlinks (`/usr/bin/opt-22` ->
/// `/usr/lib/llvm-22/bin/opt` -> `/usr/lib/llvm-22`).
fn prefix_of_tool(tool: &Path) -> Option<PathBuf> {
    let real = std::fs::canonicalize(tool).ok()?;
    Some(real.parent()?.parent()?.to_path_buf())
}

/// Whether `dir` is a valid installation; its major if so.
pub fn check(dir: &Path) -> Result<u32, Reject> {
    if !dir.is_dir() {
        return Err(Reject::Missing);
    }
    let bin = dir.join("bin");
    if let Some(t) = REQUIRED_TOOLS.iter().find(|t| !bin.join(t).is_file()) {
        return Err(Reject::NoTool(t));
    }
    let major = dir_major(dir).ok_or(Reject::NoMajor)?;
    if SUPPORTED_LLVM_MAJORS.contains(&major) { Ok(major) } else { Err(Reject::Unsupported(major)) }
}

/// The major of the installation at `dir`, without running anything when
/// the layout tells: the one numeric entry of `lib/clang/` (clang's
/// resource directory is `lib/clang/<major>`), else the major of
/// `bin/llvm-config --version` (probed once per directory per process),
/// else the `N` of a directory named `llvm-N`, `llvmN` or `llvm@N`
/// (before or after resolving symlinks).
pub fn dir_major(dir: &Path) -> Option<u32> {
    clang_resource_major(dir)
        .or_else(|| probe_llvm_config_major(dir))
        .or_else(|| dir_name_major(dir))
        .or_else(|| std::fs::canonicalize(dir).ok().and_then(|d| dir_name_major(&d)))
}

/// The `N` of the single numeric entry of `<dir>/lib/clang`.
fn clang_resource_major(dir: &Path) -> Option<u32> {
    let mut found = None;
    for e in std::fs::read_dir(dir.join("lib").join("clang")).ok()?.flatten() {
        let Some(n) = e.file_name().to_str().and_then(|s| s.parse::<u32>().ok()) else {
            continue;
        };
        if found.is_some() {
            return None;
        }
        found = Some(n);
    }
    found
}

/// The major of `<dir>/bin/llvm-config --version`, cached per directory
/// for the life of the process. `None` when the tool is missing, fails, or
/// prints something that is not `N.M.P...`.
fn probe_llvm_config_major(dir: &Path) -> Option<u32> {
    static CACHE: Mutex<Vec<(PathBuf, Option<u32>)>> = Mutex::new(Vec::new());
    let mut cache = CACHE.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some((_, m)) = cache.iter().find(|(d, _)| d == dir) {
        return *m;
    }
    let tool = dir.join("bin").join("llvm-config");
    let m = if tool.is_file() {
        std::process::Command::new(&tool)
            .arg("--version")
            .stdin(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .output()
            .ok()
            .filter(|o| o.status.success())
            .and_then(|o| llvm_config_version_major(&String::from_utf8_lossy(&o.stdout)))
    } else {
        None
    };
    cache.push((dir.to_path_buf(), m));
    m
}

/// `22.1.8`, `19.1.7git` -> 22, 19: the leading integer of
/// `llvm-config --version`'s output.
fn llvm_config_version_major(out: &str) -> Option<u32> {
    let major = out.trim().split('.').next()?;
    if major.is_empty() || !major.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    major.parse().ok()
}

/// `/usr/lib/llvm-22`, `/usr/lib64/llvm19`, `.../llvm@19` -> 22, 19, 19;
/// `None` unless the last component is one of those shapes.
fn dir_name_major(dir: &Path) -> Option<u32> {
    let name = dir.file_name()?.to_str()?.strip_prefix("llvm")?;
    let n = name.strip_prefix(['-', '@']).unwrap_or(name);
    if n.is_empty() || !n.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    n.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supported_set_text() {
        assert_eq!(supported_text(), "19 and 22");
        assert_eq!(DEFAULT_LLVM_MAJOR, 22);
    }

    #[test]
    fn dir_name_majors() {
        assert_eq!(dir_name_major(Path::new("/usr/lib/llvm-22")), Some(22));
        assert_eq!(dir_name_major(Path::new("/usr/lib/llvm-19/")), Some(19));
        assert_eq!(dir_name_major(Path::new("/usr/lib64/llvm19")), Some(19));
        assert_eq!(dir_name_major(Path::new("/opt/llvm@22")), Some(22));
        assert_eq!(dir_name_major(Path::new("/opt/llvm")), None);
        assert_eq!(dir_name_major(Path::new("/opt/llvm-main")), None);
    }

    #[test]
    fn llvm_config_versions() {
        assert_eq!(llvm_config_version_major("22.1.8\n"), Some(22));
        assert_eq!(llvm_config_version_major("19.1.7"), Some(19));
        assert_eq!(llvm_config_version_major("23.0.0git\n"), Some(23));
        assert_eq!(llvm_config_version_major(""), None);
        assert_eq!(llvm_config_version_major("x.1"), None);
    }

    use std::os::unix::fs::{PermissionsExt, symlink};

    /// A scratch directory, removed when dropped.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Scratch {
            let d = std::env::temp_dir()
                .join(format!("kiln-llvm-search-{}-{name}", std::process::id()));
            let _ = std::fs::remove_dir_all(&d);
            std::fs::create_dir_all(&d).unwrap();
            // Tools on PATH resolve to canonical paths.
            Scratch(std::fs::canonicalize(&d).unwrap())
        }

        fn path(&self, rel: &str) -> PathBuf {
            self.0.join(rel.trim_start_matches('/'))
        }

        fn root(&self) -> String {
            self.0.display().to_string()
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn script(path: &Path, body: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    /// How a fake installation tells its major.
    #[derive(Clone, Copy)]
    enum Says {
        /// `lib/clang/<N>`.
        ResourceDir(u32),
        /// A `bin/llvm-config` stub printing `<N>.1.0`.
        LlvmConfig(u32),
        /// Nothing (only its name may tell).
        Nothing,
    }

    /// A fake LLVM installation at `dir`: stub `opt`, `llc` and `clang++`
    /// (each but `skip`) that report `says`' major.
    fn fake_skipping(dir: &Path, says: Says, skip: Option<&str>) {
        let v = match says {
            Says::ResourceDir(n) | Says::LlvmConfig(n) => n,
            Says::Nothing => 0,
        };
        for t in REQUIRED_TOOLS.iter().filter(|t| Some(**t) != skip) {
            script(&dir.join("bin").join(t), &format!("echo \"{t} version {v}.1.0\""));
        }
        match says {
            Says::ResourceDir(n) => {
                std::fs::create_dir_all(dir.join("lib/clang").join(n.to_string())).unwrap();
            }
            Says::LlvmConfig(n) => script(&dir.join("bin/llvm-config"), &format!("echo {n}.1.0")),
            Says::Nothing => {}
        }
    }

    fn fake(dir: &Path, says: Says) {
        fake_skipping(dir, says, None);
    }

    /// A search with nothing explicit, the layouts under `s`, and `path`
    /// as `PATH` (scratch-relative directories).
    fn search(s: &Scratch, path: &[&str]) -> Search {
        let path = (!path.is_empty())
            .then(|| std::env::join_paths(path.iter().map(|p| s.path(p))).unwrap());
        Search { root: s.root(), path, ..Search::default() }
    }

    fn found(r: &LlvmSelection) -> (PathBuf, u32, Origin) {
        let i = r.as_ref().unwrap_or_else(|e| panic!("{e:?}"));
        (i.dir.clone(), i.major, i.origin.clone())
    }

    // spec/driver.md "LLVM discovery": `--llvm-dir=` beats
    // `KILN_LLVM_DIR`, which beats every implicit candidate.
    #[test]
    fn precedence_flag_env_search() {
        let s = Scratch::new("precedence");
        fake(&s.path("flag"), Says::ResourceDir(19));
        fake(&s.path("env"), Says::LlvmConfig(22));
        fake(&s.path("/usr/lib/llvm-22"), Says::ResourceDir(22));
        let flag = Some(s.path("flag").display().to_string());
        let env = Some(s.path("env").display().to_string());
        let base = search(&s, &[]);
        let both = Search { flag: flag.clone(), env: env.clone(), ..base.clone() };
        assert_eq!(found(&both.run()), (s.path("flag"), 19, Origin::Flag));
        let env_only = Search { env, ..base.clone() };
        assert_eq!(found(&env_only.run()), (s.path("env"), 22, Origin::Env));
        assert_eq!(found(&base.run()), (s.path("/usr/lib/llvm-22"), 22, Origin::Layout));
        // The build default comes before the search.
        let built = Search { build_default: flag, ..base };
        assert_eq!(found(&built.run()), (s.path("flag"), 19, Origin::BuildDefault));
    }

    // An explicit choice that is not an installation is an error naming
    // it, even when a later candidate is valid.
    #[test]
    fn explicit_invalid_is_an_error() {
        let s = Scratch::new("invalid");
        fake(&s.path("/usr/lib/llvm-22"), Says::ResourceDir(22));
        fake_skipping(&s.path("nollc"), Says::ResourceDir(22), Some("llc"));
        fake(&s.path("noversion"), Says::Nothing);
        let base = search(&s, &[]);
        for (dir, origin, reason) in [
            ("missing", Origin::Flag, Reject::Missing),
            ("nollc", Origin::Env, Reject::NoTool("llc")),
            ("noversion", Origin::Flag, Reject::NoMajor),
        ] {
            let d = Some(s.path(dir).display().to_string());
            let srch = match origin {
                Origin::Flag => Search { flag: d, ..base.clone() },
                _ => Search { env: d, ..base.clone() },
            };
            assert_eq!(
                srch.run(),
                Err(LlvmError::Invalid { dir: s.path(dir), origin, reason }),
                "{dir}"
            );
        }
        // A bad flag does not fall through to a good KILN_LLVM_DIR.
        let srch = Search {
            flag: Some(s.path("missing").display().to_string()),
            env: Some(s.path("/usr/lib/llvm-22").display().to_string()),
            ..base
        };
        assert!(matches!(srch.run(), Err(LlvmError::Invalid { origin: Origin::Flag, .. })));
    }

    // LLVM 20 and 21 are refused by name, from either way of telling the
    // major; the error carries the major found.
    #[test]
    fn explicit_unsupported_major() {
        let s = Scratch::new("unsupported");
        fake(&s.path("llvm21"), Says::LlvmConfig(21));
        fake(&s.path("llvm20"), Says::ResourceDir(20));
        for (dir, major) in [("llvm21", 21), ("llvm20", 20)] {
            let srch = Search { flag: Some(s.path(dir).display().to_string()), ..search(&s, &[]) };
            assert_eq!(
                srch.run(),
                Err(LlvmError::Unsupported { dir: s.path(dir), origin: Origin::Flag, major })
            );
        }
        // A directory named for an unsupported major with no other sign.
        fake(&s.path("llvm-18"), Says::Nothing);
        let srch = Search { env: Some(s.path("llvm-18").display().to_string()), ..search(&s, &[]) };
        assert!(matches!(srch.run(), Err(LlvmError::Unsupported { major: 18, .. })));
    }

    // 22 is preferred over 19 wherever each is found; an invalid or
    // unsupported implicit candidate is skipped (and listed if nothing
    // is found).
    #[test]
    fn implicit_prefers_22_and_skips_invalid() {
        let s = Scratch::new("implicit");
        fake(&s.path("/usr/lib/llvm-19"), Says::ResourceDir(19));
        assert_eq!(found(&search(&s, &[]).run()).0, s.path("/usr/lib/llvm-19"));
        // An unversioned llvm-config of LLVM 19 does not beat LLVM 22's
        // layout.
        fake(&s.path("/opt/old"), Says::ResourceDir(19));
        script(&s.path("/opt/old/bin/llvm-config"), "echo 19.1.7");
        fake(&s.path("/usr/lib/llvm-22"), Says::ResourceDir(22));
        let r = search(&s, &["/opt/old/bin"]).run();
        assert_eq!(found(&r), (s.path("/usr/lib/llvm-22"), 22, Origin::Layout));
        // A 22 missing clang++ is skipped for 19's llvm-config.
        std::fs::remove_file(s.path("/usr/lib/llvm-22/bin/clang++")).unwrap();
        let r = search(&s, &["/opt/old/bin"]).run();
        let llvm_config = s.path("/opt/old/bin/llvm-config");
        assert_eq!(found(&r), (s.path("/opt/old"), 19, Origin::LlvmConfig(llvm_config)));
        // An unsupported Fedora-style /usr is skipped.
        fake(&s.path("/usr"), Says::LlvmConfig(21));
        let r = search(&s, &[]).run();
        assert_eq!(found(&r).0, s.path("/usr/lib/llvm-19"));
    }

    // `llvm-config-<N>` and `opt-<N>` / `clang++-<N>` on PATH name the
    // installation they resolve into.
    #[test]
    fn path_tools_name_their_installation() {
        let s = Scratch::new("path");
        fake(&s.path("/opt/llvm22"), Says::Nothing);
        std::fs::create_dir_all(s.path("/bin")).unwrap();
        symlink(s.path("/opt/llvm22/bin/opt"), s.path("/bin/opt-22")).unwrap();
        let r = search(&s, &["/bin"]).run();
        assert_eq!(found(&r), (s.path("/opt/llvm22"), 22, Origin::PathTool(s.path("/bin/opt-22"))));
        // llvm-config-22 comes before the layouts and the other tools.
        fake(&s.path("/usr/lib/llvm-22"), Says::ResourceDir(22));
        fake(&s.path("/opt/x"), Says::LlvmConfig(22));
        symlink(s.path("/opt/x/bin/llvm-config"), s.path("/bin/llvm-config-22")).unwrap();
        let r = search(&s, &["/bin"]).run();
        assert_eq!(found(&r).0, s.path("/opt/x"));
    }

    // Nothing valid: every directory tried with why, then the PATH names
    // not found.
    #[test]
    fn nothing_found_lists_what_was_tried() {
        let s = Scratch::new("none");
        fake_skipping(&s.path("/usr/lib/llvm-22"), Says::ResourceDir(22), Some("opt"));
        let Err(LlvmError::NotFound { tried, not_on_path }) = search(&s, &[]).run() else {
            panic!("expected nothing found")
        };
        assert_eq!(tried[0], (s.path("/usr/lib/llvm-22"), Reject::NoTool("opt")));
        assert_eq!(tried[1], (s.path("/usr/lib64/llvm22"), Reject::Missing));
        assert!(tried.contains(&(s.path("/usr/lib/llvm-19"), Reject::Missing)));
        assert_eq!(
            not_on_path,
            [
                "llvm-config",
                "llvm-config-22",
                "opt-22",
                "clang++-22",
                "llvm-config-19",
                "opt-19",
                "clang++-19"
            ]
        );
    }

    fn rendered(e: &LlvmError) -> String {
        let sm = kiln_source::SourceManager::new();
        let buf = kiln_diag::SharedBuffer::new();
        let err = crate::SharedWriter::new(buf.clone());
        let drv = crate::make_engine(&sm, &kiln_diag::DiagOptions::default(), err, Some("kiln++"));
        e.emit(&drv);
        drv.flush();
        buf.contents()
    }

    #[test]
    fn error_text() {
        let e =
            LlvmError::Unsupported { dir: "/opt/llvm-21".into(), origin: Origin::Env, major: 21 };
        assert_eq!(
            rendered(&e),
            "kiln++: error: unsupported LLVM major 21 at '/opt/llvm-21' (from KILN_LLVM_DIR); \
             kiln supports LLVM 19 and 22\n"
        );
        let e = LlvmError::Invalid {
            dir: "/x".into(),
            origin: Origin::Flag,
            reason: Reject::NoTool("clang++"),
        };
        assert_eq!(
            rendered(&e),
            "kiln++: error: '/x' (from --llvm-dir) is not a usable LLVM installation: \
             no 'bin/clang++'\n"
        );
        let e = LlvmError::NotFound {
            tried: vec![("/usr/lib/llvm-22".into(), Reject::Missing)],
            not_on_path: vec!["llvm-config".into(), "opt-22".into()],
        };
        assert_eq!(
            rendered(&e),
            "kiln++: error: no supported LLVM installation found (kiln supports LLVM 19 and 22)\n\
             kiln++: note: skipped '/usr/lib/llvm-22': no such directory\n\
             kiln++: note: not found on PATH: llvm-config, opt-22\n\
             kiln++: note: install LLVM 22 or 19 with clang (docs/COMPATIBILITY.md \
             \"Runtime dependencies\"), or name an installation with --llvm-dir=<dir> or \
             KILN_LLVM_DIR\n"
        );
    }
}
