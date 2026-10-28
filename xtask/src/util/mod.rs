//! Pieces shared by the `cargo xtask` subcommands: locating the repository and
//! the cargo target dir, building and identifying compilers, running child
//! processes, the `//@` directive header, and JSON.

pub mod header;
pub mod json;
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

/// `cargo build [--release] -p kiln_driver`, returning the path of the `kiln`
/// binary. `who` names the subcommand in the progress message.
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
    let exe = target.join(profile).join(format!("kiln{}", std::env::consts::EXE_SUFFIX));
    if exe.is_file() { Ok(exe) } else { Err(format!("kiln binary not found at {}", exe.display())) }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
