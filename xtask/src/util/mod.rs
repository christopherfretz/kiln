//! Pieces shared by the `cargo xtask` subcommands: locating the repository and
//! the cargo target dir, building and identifying compilers, running child
//! processes, the `//@` directive header, and JSON.

pub mod header;
pub mod json;
pub mod oven_report;
pub mod process;

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use json::Json;

/// Which compiler family a binary belongs to, from `--version`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CompilerKind {
    Kiln,
    Clang,
    /// GCC (`g++ (GCC) 16.1.0`): the reflection oracle (`oracle: gcc`).
    Gcc,
    Other,
}

impl CompilerKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Kiln => "kiln",
            Self::Clang => "clang",
            Self::Gcc => "gcc",
            Self::Other => "other",
        }
    }

    /// Classifies the first line of `--version` output: `kiln version ...`
    /// is kiln, anything containing `clang version` is clang, and a line
    /// naming `(GCC)` or starting with `g++`/`gcc` (`g++-14 (Debian
    /// 14.2.0-19) 14.2.0`) is GCC.
    pub fn from_version_line(first: &str) -> Self {
        if first.starts_with("kiln version") {
            Self::Kiln
        } else if first.contains("clang version") {
            Self::Clang
        } else if first.contains("(GCC)") || first.starts_with("g++") || first.starts_with("gcc") {
            Self::Gcc
        } else {
            Self::Other
        }
    }

    /// Does this compiler take clang's driver flags and dumps (`-Xclang
    /// -verify`, `-fmodule-file=`, `-Xclang -ast-dump`)? kiln and clang do.
    pub fn speaks_clang(self) -> bool {
        matches!(self, Self::Kiln | Self::Clang)
    }
}

/// A compiler binary and what `--version` says it is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompilerId {
    pub kind: CompilerKind,
    /// The first line of `--version`, trimmed.
    pub version: String,
}

impl CompilerId {
    /// The `X.Y.Z` after `clang version ` (`Debian clang version 19.1.7
    /// (3+b1)` is 19.1.7), for `requires: clang>=`; `None` for kiln and
    /// for a line that names no version.
    pub fn clang_version(&self) -> Option<ClangVersion> {
        if self.kind != CompilerKind::Clang {
            return None;
        }
        let rest = &self.version[self.version.find("clang version ")? + "clang version ".len()..];
        let v = rest.split(|c: char| !(c.is_ascii_digit() || c == '.')).next()?;
        // `19.1.7git`, `20.0.0` and `19` all parse; missing parts are 0.
        parse_version(v.trim_end_matches('.'))
    }
}

/// A clang release, `[major, minor, patch]`, as `requires: clang>=` and the
/// `clang version X.Y.Z` of `--version` spell it.
pub type ClangVersion = [u32; 3];

/// `X[.Y[.Z]]`, missing components 0; `None` unless every component is a
/// decimal number.
pub fn parse_version(text: &str) -> Option<ClangVersion> {
    let mut v = [0; 3];
    let mut parts = text.split('.');
    for slot in &mut v {
        match parts.next() {
            Some(p) if !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()) => {
                *slot = p.parse().ok()?;
            }
            Some(_) => return None,
            None => break,
        }
    }
    parts.next().is_none().then_some(v)
}

/// Runs `<compiler> --version` and classifies the first line.
pub fn identify(compiler: &Path) -> Result<CompilerId, String> {
    let o = process::run(Command::new(compiler).arg("--version"), Duration::from_secs(30))
        .map_err(|e| format!("cannot run compiler `{}`: {e}", compiler.display()))?;
    if !o.success() {
        return Err(format!("`{} --version` failed ({})", compiler.display(), o.describe_end()));
    }
    let version = o.stdout_str().lines().next().unwrap_or_default().trim().to_owned();
    Ok(CompilerId { kind: CompilerKind::from_version_line(&version), version })
}

/// The repository root (the `xtask` crate lives in `<root>/xtask`).
pub fn repo_root() -> PathBuf {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest.parent().unwrap_or(manifest).to_path_buf()
}

/// `$CARGO_TARGET_DIR`, else `target_directory` from `cargo metadata`.
pub fn target_dir(root: &Path) -> Result<PathBuf, String> {
    if let Some(d) = std::env::var_os("CARGO_TARGET_DIR").filter(|d| !d.is_empty()) {
        return Ok(root.join(d));
    }
    let o = Command::new(env!("CARGO"))
        .args(["metadata", "--format-version", "1", "--no-deps"])
        .current_dir(root)
        .output()
        .map_err(|e| format!("cannot run cargo metadata: {e}"))?;
    if !o.status.success() {
        return Err(format!("cargo metadata failed: {}", String::from_utf8_lossy(&o.stderr)));
    }
    json_string_field(&String::from_utf8_lossy(&o.stdout), "target_directory")
        .map(PathBuf::from)
        .ok_or_else(|| "cargo metadata: no target_directory".into())
}

/// The `XDG_CACHE_HOME` the suites give every compiler they run: a
/// directory under the target directory, so kiln's module-file cache
/// (spec/driver.md "Module cache") never writes to the user's
/// `~/.cache`, and test runs share one cache.
pub fn suite_cache_home() -> PathBuf {
    static DIR: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    DIR.get_or_init(|| {
        let root = repo_root();
        target_dir(&root).unwrap_or_else(|_| root.join("target")).join("xdg-cache")
    })
    .clone()
}

/// Points `c`'s caches at [`suite_cache_home`].
pub fn isolate_caches(c: &mut Command) -> &mut Command {
    c.env("XDG_CACHE_HOME", suite_cache_home())
}

/// The directory of libc++'s module sources (`std.cppm`) as `compiler`
/// reports them: the manifest `-print-file-name=libc++.modules.json`
/// names, else (clang prints the bare name when its own prefix has no
/// libc++, as Debian's clang 22 beside libc++ 19) the manifest beside the
/// libc++ headers its `-v` search list shows, `<prefix>/include/c++/v1`
/// with symlinks resolved giving `<prefix>/lib/libc++.modules.json` (the
/// rule kiln itself uses). Then the `std` entry's `source-path`, relative
/// to the manifest. `None` when neither names an existing `std.cppm`.
pub fn libcxx_module_dir_of(compiler: &Path, llvm_dir: &Path) -> Option<PathBuf> {
    let run = |args: &[&str]| {
        Command::new(compiler).args(args).env("KILN_LLVM_DIR", llvm_dir).output().ok()
    };
    let printed = run(&["-stdlib=libc++", "-print-file-name=libc++.modules.json"])
        .filter(|o| o.status.success())
        .map(|o| PathBuf::from(String::from_utf8_lossy(&o.stdout).trim()))
        .filter(|p| p.is_absolute() && p.is_file());
    let json = printed.or_else(|| {
        let o = run(&["-stdlib=libc++", "-x", "c++", "-E", "-v", "/dev/null", "-o", "/dev/null"])?;
        let err = String::from_utf8_lossy(&o.stderr);
        let v1 = err.lines().map(str::trim).find(|l| l.ends_with("/c++/v1"))?;
        let v1 = std::fs::canonicalize(v1).ok()?;
        let json = v1.parent()?.parent()?.parent()?.join("lib").join("libc++.modules.json");
        json.is_file().then_some(json)
    })?;
    let text = std::fs::read_to_string(&json).ok()?;
    let doc = Json::parse(&text).ok()?;
    let modules = doc.get("modules")?.as_arr()?;
    let std = modules.iter().find(|m| m.get("logical-name").and_then(Json::as_str) == Some("std"))?;
    let source = std.get("source-path")?.as_str()?;
    let path = std::fs::canonicalize(json.parent()?.join(source)).ok()?;
    path.parent().map(Path::to_path_buf)
}

/// The string value of the top-level `key` of a JSON object document.
pub fn json_string_field(json: &str, key: &str) -> Option<String> {
    Json::parse(json).ok()?.get(key)?.as_str().map(str::to_owned)
}

/// The cargo profile a suite driver builds kiln with (`--profile`,
/// spec/libcxx.md "Build profiles").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Profile {
    /// `dev`: what `cargo build` makes; cheapest to build, slowest to run.
    Debug,
    /// `harness`: optimized, no LTO, 16 codegen units (Cargo.toml).
    Harness,
    /// `release`: thin LTO, 4 codegen units; the bench baseline.
    Release,
}

impl Profile {
    /// The `--profile` spellings, in the order the usage text lists them.
    pub const NAMES: &str = "debug|harness|release";

    pub fn parse(s: &str) -> Result<Self, String> {
        match s {
            "debug" | "dev" => Ok(Self::Debug),
            "harness" => Ok(Self::Harness),
            "release" => Ok(Self::Release),
            _ => Err(format!("unknown --profile `{s}` (known: {})", Self::NAMES)),
        }
    }

    /// The name reports and status rows use.
    pub fn label(self) -> &'static str {
        match self {
            Self::Debug => "debug",
            Self::Harness => "harness",
            Self::Release => "release",
        }
    }

    /// The name `cargo build --profile` takes.
    pub fn cargo_name(self) -> &'static str {
        match self {
            Self::Debug => "dev",
            other => other.label(),
        }
    }

    /// The directory under the target dir the binaries land in.
    pub fn dir(self) -> &'static str {
        self.label()
    }

    /// The default limit for one compile or link step: an unoptimized kiln
    /// is several times slower on template-heavy tests (`variant.visit`).
    pub fn compile_timeout(self) -> Duration {
        match self {
            Self::Debug => Duration::from_secs(300),
            Self::Harness | Self::Release => Duration::from_secs(120),
        }
    }
}

/// `cargo build [--release] -p kiln_driver`, returning the path of the
/// `kiln++` binary (the C++ driver mode: spec/driver.md "Driver modes").
/// `who` names the subcommand in the progress message.
pub fn build_kiln(root: &Path, target: &Path, who: &str, release: bool) -> Result<PathBuf, String> {
    build_kiln_with(root, target, who, if release { Profile::Release } else { Profile::Debug })
}

/// `cargo build --profile <p> -p kiln_driver`, returning the path of the
/// `kiln++` binary.
pub fn build_kiln_with(
    root: &Path,
    target: &Path,
    who: &str,
    profile: Profile,
) -> Result<PathBuf, String> {
    eprintln!(
        "xtask {who}: building kiln (cargo build --profile {} -p kiln_driver)",
        profile.cargo_name()
    );
    // In the live-children table: an interrupted xtask stops it too.
    let mut cmd = Command::new(env!("CARGO"));
    cmd.args(["build", "--quiet", "-p", "kiln_driver", "--profile", profile.cargo_name()])
        .current_dir(root);
    let (status, _) =
        process::run_step(&mut cmd, false).map_err(|e| format!("cannot run cargo build: {e}"))?;
    if !status.success() {
        return Err(format!("cargo build -p kiln_driver failed ({status})"));
    }
    kilnpp_beside(&target.join(profile.dir()))
}

/// `<dir>/kiln++`, made as a symlink to `<dir>/kiln` if it is missing
/// (`kiln_driver`'s build script makes it once per build directory).
pub fn kilnpp_beside(dir: &Path) -> Result<PathBuf, String> {
    let exe = std::env::consts::EXE_SUFFIX;
    let kiln = dir.join(format!("kiln{exe}"));
    if !kiln.is_file() {
        return Err(format!("kiln binary not found at {}", kiln.display()));
    }
    let kilnpp = dir.join(format!("kiln++{exe}"));
    if !kilnpp.is_file() {
        let _ = std::fs::remove_file(&kilnpp); // a dangling link
        link_to(&kilnpp, &format!("kiln{exe}"))
            .map_err(|e| format!("cannot make {}: {e}", kilnpp.display()))?;
    }
    Ok(kilnpp)
}

#[cfg(unix)]
fn link_to(link: &Path, target: &str) -> std::io::Result<()> {
    std::os::unix::fs::symlink(target, link)
}

#[cfg(not(unix))]
fn link_to(link: &Path, target: &str) -> std::io::Result<()> {
    std::fs::copy(link.with_file_name(target), link).map(drop)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kilnpp_is_made_beside_kiln() {
        let d = std::env::temp_dir().join(format!("xtask-kilnpp-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        assert!(kilnpp_beside(&d).unwrap_err().starts_with("kiln binary not found"));
        std::fs::write(d.join("kiln"), "#!/bin/sh\n").unwrap();
        let p = kilnpp_beside(&d).unwrap();
        assert_eq!(p, d.join("kiln++"));
        assert!(p.is_file());
        assert_eq!(kilnpp_beside(&d).unwrap(), p, "an existing kiln++ is kept");
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn compiler_kind_from_version() {
        assert_eq!(
            CompilerKind::from_version_line("kiln version 0.1.0 (C++26 frontend)"),
            CompilerKind::Kiln
        );
        assert_eq!(
            CompilerKind::from_version_line("Ubuntu clang version 19.1.7 (++2025)"),
            CompilerKind::Clang
        );
        assert_eq!(CompilerKind::from_version_line("g++ (GCC) 16.1.0"), CompilerKind::Gcc);
        assert_eq!(
            CompilerKind::from_version_line("g++-14 (Debian 14.2.0-19) 14.2.0"),
            CompilerKind::Gcc
        );
        assert_eq!(CompilerKind::from_version_line("icpx (R) 2025"), CompilerKind::Other);
        assert!(!CompilerKind::Gcc.speaks_clang() && CompilerKind::Kiln.speaks_clang());
    }

    /// WP-239: the version `requires: clang>=` compares against.
    #[test]
    fn clang_version_from_version_line() {
        let id = |line: &str| CompilerId {
            kind: CompilerKind::from_version_line(line),
            version: line.to_owned(),
        };
        assert_eq!(id("Debian clang version 19.1.7 (3+b1)").clang_version(), Some([19, 1, 7]));
        assert_eq!(
            id("Ubuntu clang version 19.1.1 (1ubuntu1~24.04.2)").clang_version(),
            Some([19, 1, 1])
        );
        assert_eq!(id("clang version 20.0.0git (https://x y)").clang_version(), Some([20, 0, 0]));
        assert_eq!(id("clang version 19").clang_version(), Some([19, 0, 0]));
        assert_eq!(id("clang version unknown").clang_version(), None);
        assert_eq!(id("kiln version 0.1.0 (clang version 19.1.7)").clang_version(), None);
        assert_eq!(id("g++ (GCC) 14.2").clang_version(), None);
        assert_eq!(parse_version("19.1"), Some([19, 1, 0]));
        assert_eq!(parse_version(""), None);
    }
}
