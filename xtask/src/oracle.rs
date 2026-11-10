//! The oracle toolchain: the one place that names the clang/LLVM release the
//! harness compares kiln with (`spec/harness.md` "Oracle toolchain").
//!
//! `KILN_ORACLE` selects the release: `22` (the default since WP-355) or
//! `19` (kiln's codegen and the M3 libc++ gate still mirror it; CI runs the
//! quick suite under both). Every tool name (`clang++-N`, `opt-N`, `llc-N`, `FileCheck-N`),
//! the LLVM directory `/usr/lib/llvm-N` (exported to kiln++ as
//! `KILN_LLVM_DIR`) and the libc++ header directory come from here; the
//! commands' `--oracle`/`--compiler` flags still override the compiler.
//!
//! The GCC oracle (`oracle: gcc` tests; `spec/harness.md` "GCC oracle") is
//! named by `KILN_GCC` (default `g++-16` on `PATH`); [`gcc_args`] maps a
//! test's kiln/clang flags to GCC's. `KILN_REQUIRE_GCC=1` (GitHub CI) makes
//! a missing GCC an infrastructure error instead of dropped comparisons.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

/// The environment variable that names the GCC oracle.
pub const GCC_ENV: &str = "KILN_GCC";

/// The GCC an unset (or empty) `KILN_GCC` means, looked up on `PATH`.
pub const GCC_DEFAULT: &str = "g++-16";

/// The GCC oracle a `KILN_GCC` value names, and whether it was named
/// explicitly (an explicit GCC that cannot run is an error; the default
/// one missing just disables the GCC comparisons).
pub fn gcc_from(value: Option<&str>) -> (PathBuf, bool) {
    match value.map(str::trim).filter(|v| !v.is_empty()) {
        Some(v) => (PathBuf::from(v), true),
        None => (PathBuf::from(GCC_DEFAULT), false),
    }
}

/// The environment variable that makes the GCC oracle mandatory.
pub const REQUIRE_GCC_ENV: &str = "KILN_REQUIRE_GCC";

/// Whether a `KILN_REQUIRE_GCC` value requires the GCC oracle: exactly `1`,
/// as `KILN_REQUIRE_LLVM`. Only the command entry points read it
/// ([`require_gcc_from_env`]), so the harness's own tests never see CI's.
pub fn require_gcc_from(value: Option<&str>) -> bool {
    value.map(str::trim) == Some("1")
}

/// [`require_gcc_from`] of the environment.
pub fn require_gcc_from_env() -> bool {
    require_gcc_from(std::env::var(REQUIRE_GCC_ENV).ok().as_deref())
}

/// A test's `args:` (placeholders already substituted) as GCC 16 takes
/// them: kiln- and clang-only flags are dropped (`-fclang-compat=`,
/// `-fkiln-*`, `-fconstexpr-report=`, `-stdlib=`, `-resource-dir`,
/// `-Xclang <arg>`, `--gcc-toolchain=`: GCC is its own toolchain), and the two limits GCC spells differently are
/// renamed (`-ferror-limit=N` is `-fmax-errors=N`, `-fconstexpr-steps=N`
/// is `-fconstexpr-ops-limit=N`). clang's diagnostic-only flags, which GCC
/// has no equivalent for and which change neither a verdict nor a binary,
/// are dropped too (WP-1261): `-Qunused-arguments`, the `-R<remark>`
/// remarks, `-f[no-]crash-diagnostics[=...]` and `-fcrash-diagnostics-dir=`,
/// and `-Weverything`; `-Wno-everything` is GCC's `-w`. `-freflection`,
/// `-I`, `-D`, `-W...` and the rest pass through: GCC 16.1 spells
/// reflection `-std=c++26 -freflection` as kiln does. [`gcc_args_with`]
/// also drops the warning flags a given GCC does not know.
pub fn gcc_args(args: &[String]) -> Vec<String> {
    gcc_args_with(args, None)
}

/// [`gcc_args`], and with `warnings` (the GCC's own warning options,
/// [`GccWarnings::of`]) every `-W<name>`, `-Wno-<name>`, `-Werror=<name>`
/// and `-Wno-error=<name>` whose `<name>` that GCC does not know is dropped
/// (clang's warning groups, such as `-Wshadow-all`: GCC rejects an unknown
/// `-W` option). Warnings change no verdict except under `-Werror`, and a
/// clang group's `-Werror=` has nothing to promote in GCC.
pub fn gcc_args_with(args: &[String], warnings: Option<&GccWarnings>) -> Vec<String> {
    let mut out = Vec::with_capacity(args.len());
    let mut it = args.iter();
    while let Some(a) = it.next() {
        if a == "-Xclang" || a == "-resource-dir" {
            it.next();
        } else if let Some(n) = a.strip_prefix("-ferror-limit=") {
            out.push(format!("-fmax-errors={n}"));
        } else if let Some(n) = a.strip_prefix("-fconstexpr-steps=") {
            out.push(format!("-fconstexpr-ops-limit={n}"));
        } else if a == "-Wno-everything" {
            out.push("-w".to_owned());
        } else if !(a.starts_with("-fclang-compat=")
            || a.starts_with("-fkiln-")
            || a.starts_with("-fconstexpr-report")
            || a.starts_with("-stdlib=")
            || a.starts_with("-resource-dir=")
            || a.starts_with("--gcc-toolchain=")
            || clang_diagnostic_only(a)
            || warnings.is_some_and(|w| w.unknown(a)))
        {
            out.push(a.clone());
        }
    }
    out
}

/// A clang flag that only shapes diagnostics (or crash reports), with no
/// GCC equivalent ([`gcc_args`]).
fn clang_diagnostic_only(a: &str) -> bool {
    a == "-Qunused-arguments"
        || a == "-Weverything"
        || a == "-fno-crash-diagnostics"
        || a.starts_with("-fcrash-diagnostics")
        || a.strip_prefix("-R").is_some_and(|r| r.starts_with(|c: char| c.is_ascii_lowercase()))
}

/// The warning options a GCC knows, by name (`-W<name>` and `-W<name>=`
/// alike): the ones `<gcc> -Q --help=warnings -fsyntax-only -x <lang>
/// /dev/null` lists for the test's language (an `[available in ...]` line
/// is another language's, which GCC warns about and `-Werror` rejects).
#[derive(Debug, Default)]
pub struct GccWarnings(std::collections::BTreeSet<String>);

impl GccWarnings {
    /// The options of a `-Q --help=warnings` listing.
    pub fn parse(help: &str) -> Self {
        let names = help
            .lines()
            .filter(|l| !l.contains("[available in "))
            .filter_map(|l| l.split_whitespace().next()?.strip_prefix("-W"))
            .map(|n| n.split(['=', '<']).next().unwrap_or(n))
            .filter(|n| !n.is_empty())
            .map(str::to_owned)
            .collect();
        GccWarnings(names)
    }

    /// The warning options the GCC at `gcc` knows for C (`c`) or C++,
    /// asked once per process, path and language; `None` when it cannot be
    /// run or lists none (then no warning flag is dropped).
    pub fn of(gcc: &Path, c: bool) -> Option<Arc<GccWarnings>> {
        type Cache = Mutex<HashMap<(PathBuf, bool), Option<Arc<GccWarnings>>>>;
        static CACHE: OnceLock<Cache> = OnceLock::new();
        let cache = CACHE.get_or_init(Default::default);
        let mut map = cache.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        map.entry((gcc.to_path_buf(), c))
            .or_insert_with(|| {
                let lang = if c { "c" } else { "c++" };
                let o = std::process::Command::new(gcc)
                    .args(["-Q", "--help=warnings", "-fsyntax-only", "-x", lang, "/dev/null"])
                    .output()
                    .ok()?;
                let w = GccWarnings::parse(&String::from_utf8_lossy(&o.stdout));
                (!w.0.is_empty()).then(|| Arc::new(w))
            })
            .clone()
    }

    /// Whether `a` is a `-W` warning flag naming an option this GCC does
    /// not know. `-Werror`, `-Wno-error`, `-W` and the `-Wl,`/`-Wa,`/`-Wp,`
    /// pass-throughs are not warning names.
    pub fn unknown(&self, a: &str) -> bool {
        let Some(w) = a.strip_prefix("-W") else { return false };
        if w.is_empty() || w == "error" || w == "no-error" || w.as_bytes().get(1) == Some(&b',') {
            return false;
        }
        let name = w
            .strip_prefix("error=")
            .or_else(|| w.strip_prefix("no-error="))
            .or_else(|| w.strip_prefix("no-"))
            .unwrap_or(w);
        let base = |n: &'_ str| n.split('=').next().unwrap_or(n).to_owned();
        !self.0.contains(&base(name)) && !self.0.contains(&base(w))
    }
}

/// The installation prefix of a GCC, from its `-print-search-dirs`
/// output: the `install:` directory (`<prefix>/lib/gcc/<triple>/<version>/`)
/// four levels up, not yet canonicalized. What clang's (and kiln's)
/// `--gcc-toolchain=` takes. `None` without an `install:` line of that depth.
pub fn gcc_install_prefix(search_dirs: &str) -> Option<PathBuf> {
    let line = search_dirs.lines().find_map(|l| l.strip_prefix("install:"))?;
    let mut p = PathBuf::from(line.trim());
    for _ in 0..4 {
        if !p.pop() {
            return None;
        }
    }
    (!p.as_os_str().is_empty()).then_some(p)
}

/// [`gcc_install_prefix`] of the GCC at `gcc`, canonicalized; `None` when
/// it cannot be run or names no existing prefix.
pub fn gcc_toolchain_of(gcc: &Path) -> Option<PathBuf> {
    let o = std::process::Command::new(gcc).arg("-print-search-dirs").output().ok()?;
    let prefix = gcc_install_prefix(&String::from_utf8_lossy(&o.stdout))?;
    std::fs::canonicalize(prefix).ok().filter(|p| p.is_dir())
}

/// The environment variable that selects the oracle release.
pub const ENV: &str = "KILN_ORACLE";

/// The releases `KILN_ORACLE` accepts, oldest first.
pub const SUPPORTED: &[u32] = &[19, 22];

/// The release an unset (or empty) `KILN_ORACLE` selects.
pub const DEFAULT: u32 = 22;

/// One clang/LLVM release installed side by side with the others
/// (Debian's `-N` suffixed tools and `/usr/lib/llvm-N`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Oracle {
    /// The LLVM major version, one of [`SUPPORTED`].
    pub version: u32,
}

impl Default for Oracle {
    fn default() -> Self {
        Self { version: DEFAULT }
    }
}

impl Oracle {
    /// The release `KILN_ORACLE` names; unset (or empty) is the default.
    pub fn from_env() -> Result<Self, String> {
        Self::parse(std::env::var(ENV).ok().as_deref())
    }

    /// Parses a `KILN_ORACLE` value; `None` and `""` are the default.
    pub fn parse(value: Option<&str>) -> Result<Self, String> {
        let Some(v) = value.map(str::trim).filter(|v| !v.is_empty()) else {
            return Ok(Self::default());
        };
        match v.parse::<u32>() {
            Ok(n) if SUPPORTED.contains(&n) => Ok(Self { version: n }),
            _ => {
                let known: Vec<String> = SUPPORTED.iter().map(u32::to_string).collect();
                Err(format!(
                    "{ENV}=`{v}` is not a known oracle release (accepted: {}; unset means {})",
                    known.join(" or "),
                    DEFAULT
                ))
            }
        }
    }

    fn tool(self, name: &str) -> String {
        format!("{name}-{}", self.version)
    }

    /// The C++ compiler, `clang++-N`.
    pub fn clang(self) -> String {
        self.tool("clang++")
    }

    /// The IR optimizer/verifier, `opt-N`.
    #[allow(dead_code)] // the IR-diff corpus names its tools itself (kiln_codegen tests)
    pub fn opt(self) -> String {
        self.tool("opt")
    }

    /// The static compiler, `llc-N`.
    #[allow(dead_code)] // the IR-diff corpus names its tools itself (kiln_codegen tests)
    pub fn llc(self) -> String {
        self.tool("llc")
    }

    /// The test matcher, `FileCheck-N`.
    #[allow(dead_code)] // the IR-diff corpus names its tools itself (kiln_codegen tests)
    pub fn filecheck(self) -> String {
        self.tool("FileCheck")
    }

    /// The LLVM installation, `/usr/lib/llvm-N`; kiln++ gets it as
    /// `KILN_LLVM_DIR` (headers, `llc`, `opt`, the link driver).
    pub fn llvm_dir(self) -> PathBuf {
        PathBuf::from(format!("/usr/lib/llvm-{}", self.version))
    }

    /// The libc++ header directory, `/usr/lib/llvm-N/include/c++/v1`.
    pub fn libcxx_include(self) -> PathBuf {
        self.llvm_dir().join("include").join("c++").join("v1")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gcc_install_prefix_is_four_up_from_install() {
        let out =
            "install: /opt/g/usr/local/bin/../lib/gcc/aarch64-linux-gnu/16.1.0/\nprograms: =/x\n";
        assert_eq!(gcc_install_prefix(out), Some(PathBuf::from("/opt/g/usr/local/bin/..")));
        let distro = "install: /usr/lib/gcc/x86_64-linux-gnu/16/\n";
        assert_eq!(gcc_install_prefix(distro), Some(PathBuf::from("/usr")));
        assert_eq!(gcc_install_prefix("programs: =/x\n"), None);
        assert_eq!(gcc_install_prefix("install: /a/b\n"), None);
    }

    #[test]
    fn gcc_args_drop_gcc_toolchain() {
        let a: Vec<String> =
            ["--gcc-toolchain=/g", "-freflection"].iter().map(|s| (*s).to_owned()).collect();
        assert_eq!(gcc_args(&a), ["-freflection"]);
    }

    /// WP-1261: clang's diagnostic-only flags never reach GCC, and with a
    /// GCC's warning list the `-W` flags it does not know are dropped
    /// (spec/harness.md "GCC oracle").
    #[test]
    fn gcc_args_drop_clang_diagnostic_flags() {
        let v = |a: &[&str]| a.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
        let args = v(&[
            "-Qunused-arguments",
            "-Rpass=inline",
            "-Rauto-var-init",
            "-fno-crash-diagnostics",
            "-fcrash-diagnostics=off",
            "-fcrash-diagnostics-dir=/x",
            "-Weverything",
            "-Wno-everything",
            "-Wshadow-all",
            "-Werror=invalid-static-assert-message",
            "-Wall",
            "-Werror",
            "-Wl,--as-needed",
            "-O2",
        ]);
        let all = v(&[
            "-w",
            "-Wshadow-all",
            "-Werror=invalid-static-assert-message",
            "-Wall",
            "-Werror",
            "-Wl,--as-needed",
            "-O2",
        ]);
        assert_eq!(gcc_args(&args), all);
        let help = "The following options control compiler warning messages:\n  \
                    -Wall                       \t\t\n  -Wshadow                    \t\t[disabled]\n  \
                    -Wformat=<0,2>              \t\t0\n  \
                    -Wno-frame-larger-than      \t\t-Wframe-larger-than=18EiB\n  \
                    -Wzerotrip                  \t\t[available in Fortran]\n";
        let w = GccWarnings::parse(help);
        assert_eq!(
            gcc_args_with(&args, Some(&w)),
            v(&["-w", "-Wall", "-Werror", "-Wl,--as-needed", "-O2"])
        );
        let known = |a: &str| !w.unknown(a);
        assert!(known("-Wshadow") && known("-Wno-shadow") && known("-Werror=shadow"));
        assert!(known("-Wno-error=shadow") && known("-Wformat=2") && known("-Wno-frame-larger-than"));
        assert!(known("-Werror") && known("-Wno-error") && known("-Wa,-x") && known("-I"));
        assert!(!known("-Wshadow-all") && !known("-Wno-shadow-all") && !known("-Wzerotrip"));
        assert!(!known("-Wno-error=unknown") && !known("-Wfoo=1"));
    }

    #[test]
    fn unset_and_empty_are_22() {
        assert_eq!(Oracle::parse(None), Ok(Oracle { version: 22 }));
        assert_eq!(Oracle::parse(Some("")), Ok(Oracle { version: 22 }));
        assert_eq!(Oracle::default().version, 22);
        assert!(SUPPORTED.contains(&DEFAULT));
    }

    #[test]
    fn both_releases_parse() {
        let o19 = Oracle::parse(Some("19")).unwrap();
        assert_eq!(o19.clang(), "clang++-19");
        assert_eq!(o19.opt(), "opt-19");
        assert_eq!(o19.llc(), "llc-19");
        assert_eq!(o19.filecheck(), "FileCheck-19");
        assert_eq!(o19.llvm_dir(), PathBuf::from("/usr/lib/llvm-19"));
        assert_eq!(o19.libcxx_include(), PathBuf::from("/usr/lib/llvm-19/include/c++/v1"));
        let o22 = Oracle::parse(Some(" 22 ")).unwrap();
        assert_eq!(o22.clang(), "clang++-22");
        assert_eq!(o22.opt(), "opt-22");
        assert_eq!(o22.llc(), "llc-22");
        assert_eq!(o22.filecheck(), "FileCheck-22");
        assert_eq!(o22.llvm_dir(), PathBuf::from("/usr/lib/llvm-22"));
        assert_eq!(o22.libcxx_include(), PathBuf::from("/usr/lib/llvm-22/include/c++/v1"));
    }

    #[test]
    fn gcc_selection_and_flag_map() {
        assert_eq!(gcc_from(None), (PathBuf::from("g++-16"), false));
        assert_eq!(gcc_from(Some(" ")), (PathBuf::from("g++-16"), false));
        assert_eq!(gcc_from(Some("/opt/g++")), (PathBuf::from("/opt/g++"), true));
        assert!(require_gcc_from(Some("1")) && require_gcc_from(Some(" 1 ")));
        assert!(!require_gcc_from(None) && !require_gcc_from(Some("0")));
        assert!(!require_gcc_from(Some("")) && !require_gcc_from(Some("yes")));
        let args: Vec<String> = [
            "-freflection",
            "-I",
            "tests/support",
            "-fclang-compat=19",
            "-Xclang",
            "-verify",
            "-resource-dir",
            "/r",
            "-resource-dir=/r",
            "-stdlib=libc++",
            "-fkiln-instantiation-report=json",
            "-fconstexpr-report=/x",
            "-ferror-limit=0",
            "-fconstexpr-steps=100",
            "-fconstexpr-depth=8",
            "-DX=1",
        ]
        .map(str::to_owned)
        .to_vec();
        assert_eq!(
            gcc_args(&args),
            [
                "-freflection",
                "-I",
                "tests/support",
                "-fmax-errors=0",
                "-fconstexpr-ops-limit=100",
                "-fconstexpr-depth=8",
                "-DX=1"
            ]
        );
    }

    #[test]
    fn garbage_names_the_accepted_values() {
        for bad in ["banana", "20", "clang++-22", "-19", "19.1"] {
            let e = Oracle::parse(Some(bad)).unwrap_err();
            assert!(e.contains("KILN_ORACLE") && e.contains(bad), "{e}");
            assert!(e.contains("19 or 22"), "{e}");
        }
    }
}
