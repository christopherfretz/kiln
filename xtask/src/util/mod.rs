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
    Other,
}

impl CompilerKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Kiln => "kiln",
            Self::Clang => "clang",
            Self::Other => "other",
        }
    }

    /// Classifies the first line of `--version` output: `kiln version ...`
    /// is kiln, anything containing `clang version` is clang.
    pub fn from_version_line(first: &str) -> Self {
        if first.starts_with("kiln version") {
            Self::Kiln
        } else if first.contains("clang version") {
            Self::Clang
        } else {
            Self::Other
        }
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

/// The string value of the top-level `key` of a JSON object document.
pub fn json_string_field(json: &str, key: &str) -> Option<String> {
    Json::parse(json).ok()?.get(key)?.as_str().map(str::to_owned)
}

/// `cargo build [--release] -p kiln_driver`, returning the path of the
/// `kiln++` binary (the C++ driver mode: spec/driver.md "Driver modes").
/// `who` names the subcommand in the progress message.
pub fn build_kiln(root: &Path, target: &Path, who: &str, release: bool) -> Result<PathBuf, String> {
    let profile = if release { "release" } else { "debug" };
    let flags: &[&str] = if release { &["--release"] } else { &[] };
    eprintln!(
        "xtask {who}: building kiln (cargo build {}-p kiln_driver)",
        if release { "--release " } else { "" }
    );
    let status = Command::new(env!("CARGO"))
        .args(["build", "--quiet", "-p", "kiln_driver"])
        .args(flags)
        .current_dir(root)
        .status()
        .map_err(|e| format!("cannot run cargo build: {e}"))?;
    if !status.success() {
        return Err(format!("cargo build -p kiln_driver failed ({status})"));
    }
    kilnpp_beside(&target.join(profile))
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
        assert_eq!(CompilerKind::from_version_line("g++ (GCC) 14.2"), CompilerKind::Other);
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
