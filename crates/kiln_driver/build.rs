//! The driver's build script (spec/driver.md "Driver modes", "Version"):
//!
//! - Puts `kiln++` next to the `kiln` binary: one driver, two names, as
//!   clang and clang++. Cargo cannot build a binary target named `kiln++`
//!   (a target name must be a valid crate name), so the second name is a
//!   relative symlink `kiln++ -> kiln` in the profile directory
//!   (`target/<profile>/`, three levels above `OUT_DIR`), made once; until
//!   `kiln` is linked it dangles. xtask's `build_kiln` makes it again if
//!   it is missing, and `cargo xtask dist` makes it in the install layout.
//! - Records the git commit kiln is built from as `KILN_BUILD_GIT_SHA`
//!   (`kiln_driver::version::GIT_SHA`): `$KILN_GIT_SHA` when set (a
//!   release build from a source archive passes it), else `git rev-parse
//!   --short=12 HEAD`, but only when this crate's workspace is the root of
//!   the git work tree (a source archive unpacked inside some other
//!   repository must not take that repository's commit). Without git, or
//!   outside a work tree, nothing is recorded and the build goes on.
#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    #[cfg(unix)]
    link_kilnpp();
    git_sha();
}

#[cfg(unix)]
fn link_kilnpp() {
    let Some(out) = std::env::var_os("OUT_DIR") else { return };
    let out = PathBuf::from(out);
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

/// A short commit id: 7 to 40 hex digits.
fn valid_sha(s: &str) -> bool {
    (7..=40).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_hexdigit())
}

fn git(dir: &Path, args: &[&str]) -> Option<String> {
    let o = Command::new("git").arg("-C").arg(dir).args(args).output().ok()?;
    if !o.status.success() {
        return None;
    }
    let s = String::from_utf8(o.stdout).ok()?;
    let s = s.trim();
    (!s.is_empty()).then(|| s.to_owned())
}

fn git_sha() {
    println!("cargo:rerun-if-env-changed=KILN_GIT_SHA");
    if let Some(s) = std::env::var("KILN_GIT_SHA").ok().map(|s| s.trim().to_owned()) {
        if valid_sha(&s) {
            println!("cargo:rustc-env=KILN_BUILD_GIT_SHA={s}");
            return;
        }
        if !s.is_empty() {
            println!("cargo:warning=ignoring KILN_GIT_SHA={s:?}: not a hex commit id");
        }
    }
    let Some(manifest) = std::env::var_os("CARGO_MANIFEST_DIR").map(PathBuf::from) else {
        return;
    };
    // crates/kiln_driver -> the workspace root.
    let Some(root) = manifest.parent().and_then(Path::parent) else { return };
    let Some(top) = git(root, &["rev-parse", "--show-toplevel"]) else { return };
    let same = match (std::fs::canonicalize(&top), std::fs::canonicalize(root)) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    };
    if !same {
        return;
    }
    let Some(sha) = git(root, &["rev-parse", "--short=12", "HEAD"]).filter(|s| valid_sha(s)) else {
        return;
    };
    // Rerun when HEAD moves: HEAD itself (a checkout), the branch it names
    // (a commit) and packed-refs (a ref packed away). Only existing files:
    // a missing one would rerun the script, and rebuild the crate, on every
    // build.
    let mut watch = vec!["HEAD".to_owned(), "packed-refs".to_owned()];
    if let Some(r) = git(root, &["symbolic-ref", "-q", "HEAD"]) {
        watch.push(r);
    }
    for w in &watch {
        if let Some(p) = git(root, &["rev-parse", "--path-format=absolute", "--git-path", w])
            && Path::new(&p).is_file()
        {
            println!("cargo:rerun-if-changed={p}");
        }
    }
    println!("cargo:rustc-env=KILN_BUILD_GIT_SHA={sha}");
}
