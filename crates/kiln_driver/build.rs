//! Puts `kiln++` next to the `kiln` binary: one driver, two names, as
//! clang and clang++ (spec/driver.md "Driver modes").
//!
//! Cargo cannot build a binary target named `kiln++` (a target name must be
//! a valid crate name), so the second name is a relative symlink
//! `kiln++ -> kiln` in the profile directory (`target/<profile>/`, three
//! levels above `OUT_DIR`), made once; until `kiln` is linked it dangles.
//! xtask's `build_kiln` makes it again if it is missing.
//!
//! It also sets `KILN_GIT_SHA`, the commit the internal-compiler-error
//! report names (spec/driver.md "Internal compiler errors"): the
//! environment's `KILN_GIT_SHA` if set (packagers), else
//! `git rev-parse --short=12 HEAD`, else `unknown`.
#![forbid(unsafe_code)]

use std::path::Path;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=KILN_GIT_SHA");
    let sha = std::env::var("KILN_GIT_SHA").ok().filter(|s| !s.is_empty()).or_else(git_sha);
    println!("cargo:rustc-env=KILN_GIT_SHA={}", sha.as_deref().unwrap_or("unknown"));
    #[cfg(unix)]
    link_kilnpp();
}

/// `git <args>` in the crate directory, trimmed; `None` if git fails.
fn git(args: &[&str]) -> Option<String> {
    let dir = std::env::var_os("CARGO_MANIFEST_DIR")?;
    let out = Command::new("git").args(args).current_dir(dir).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8(out.stdout).ok()?.trim().to_owned();
    (!s.is_empty()).then_some(s)
}

/// The checkout's short commit, rebuilding when `HEAD` moves: the
/// (worktree's) `HEAD` file, the branch's ref and `packed-refs` in the
/// common directory.
fn git_sha() -> Option<String> {
    let sha = git(&["rev-parse", "--short=12", "HEAD"])?;
    let dir = git(&["rev-parse", "--absolute-git-dir"])?;
    let common = git(&["rev-parse", "--path-format=absolute", "--git-common-dir"])
        .unwrap_or_else(|| dir.clone());
    println!("cargo:rerun-if-changed={}", Path::new(&dir).join("HEAD").display());
    if let Some(r) = git(&["symbolic-ref", "-q", "HEAD"]) {
        let file = Path::new(&common).join(&r);
        if file.exists() {
            println!("cargo:rerun-if-changed={}", file.display());
        }
    }
    let packed = Path::new(&common).join("packed-refs");
    if packed.exists() {
        println!("cargo:rerun-if-changed={}", packed.display());
    }
    Some(sha)
}

#[cfg(unix)]
fn link_kilnpp() {
    let Some(out) = std::env::var_os("OUT_DIR") else { return };
    let out = std::path::PathBuf::from(out);
    // <target>/<profile>/build/kiln_driver-<hash>/out
    let Some(profile) = out.ancestors().nth(3) else { return };
    if !profile.join("build").is_dir() {
        return;
    }
    let link = profile.join("kiln++");
    if std::fs::symlink_metadata(&link).is_err() {
        // A parallel build may have made it first; either way it exists.
        let _ = std::os::unix::fs::symlink("kiln", &link);
    }
}
