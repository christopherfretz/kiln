//! LLVM discovery end to end (spec/driver.md "LLVM discovery", WP-1016):
//! `kiln++` run with fake LLVM installations whose stub `opt`, `llc` and
//! `clang++` scripts report a major, named by `--llvm-dir=` or
//! `KILN_LLVM_DIR`. Hermetic: the explicit choices stop the search before
//! it looks at the host's `/usr/lib/llvm-*` (the implicit search is unit
//! tested on fake trees in `src/llvm.rs`).
#![forbid(unsafe_code)]

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn kilnpp() -> PathBuf {
    Path::new(env!("CARGO_BIN_EXE_kiln")).with_file_name("kiln++")
}

fn scratch(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("kiln-llvm-test-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn script(path: &Path, body: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, format!("#!/bin/sh\n{body}\n")).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

/// A fake LLVM `major` installation at `dir`: `lib/clang/<major>` and stub
/// tools; its `clang++` echoes its arguments, so a delegated probe shows
/// which installation answered.
fn fake(dir: &Path, major: u32) {
    for t in ["opt", "llc"] {
        script(&dir.join("bin").join(t), &format!("echo \"{t} {major}\""));
    }
    script(&dir.join("bin/clang++"), &format!("echo \"fake clang++ {major}: $*\""));
    std::fs::create_dir_all(dir.join("lib/clang").join(major.to_string())).unwrap();
}

/// Runs `kiln++ args` in `dir` with `KILN_LLVM_DIR` set to `env` (or
/// removed).
fn kiln(dir: &Path, env: Option<&Path>, args: &[&str]) -> Output {
    let mut c = Command::new(kilnpp());
    c.args(args).current_dir(dir);
    match env {
        Some(e) => c.env("KILN_LLVM_DIR", e),
        None => c.env_remove("KILN_LLVM_DIR"),
    };
    c.output().unwrap()
}

fn text(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}

/// `--version` and `-v` name the selected LLVM, major and directory, on
/// their own line after `InstalledDir:`; nothing says "clang version".
#[test]
fn version_names_the_selected_llvm() {
    let d = scratch("version");
    fake(&d.join("llvm"), 22);
    let flag = format!("--llvm-dir={}", d.join("llvm").display());
    let want = format!("LLVM backend: 22 ({})", d.join("llvm").display());
    let o = kiln(&d, None, &["--version", &flag]);
    assert_eq!(o.status.code(), Some(0));
    let out = text(&o.stdout);
    let lines: Vec<&str> = out.lines().collect();
    assert!(lines[0].starts_with("kiln version "), "{out}");
    assert!(lines[3].starts_with("InstalledDir: "), "{out}");
    assert_eq!(lines[4], want, "{out}");
    assert!(!out.contains("clang version"), "{out}");
    let o = kiln(&d, Some(&d.join("llvm")), &["-v"]);
    assert_eq!(o.status.code(), Some(0));
    let err = text(&o.stderr);
    assert!(err.lines().any(|l| l == want), "{err}");
    assert!(!err.contains("clang version"), "{err}");
}

/// `--llvm-dir=` beats `KILN_LLVM_DIR`; the probes that delegate to the
/// linking `clang++` (WP-920) ask the selected installation's.
#[test]
fn flag_beats_env_and_probes_follow() {
    let d = scratch("precedence");
    fake(&d.join("a"), 19);
    fake(&d.join("b"), 22);
    let flag = format!("--llvm-dir={}", d.join("a").display());
    let o = kiln(&d, Some(&d.join("b")), &["-print-prog-name=ld", &flag]);
    assert_eq!(o.status.code(), Some(0), "{}", text(&o.stderr));
    assert!(text(&o.stdout).starts_with("fake clang++ 19: "), "{}", text(&o.stdout));
    assert!(text(&o.stdout).trim_end().ends_with("-print-prog-name=ld"));
    let o = kiln(&d, Some(&d.join("b")), &["-print-prog-name=ld"]);
    assert!(text(&o.stdout).starts_with("fake clang++ 22: "), "{}", text(&o.stdout));
    // The last --llvm-dir= wins.
    let other = format!("--llvm-dir={}", d.join("b").display());
    let o = kiln(&d, None, &["--version", &flag, &other]);
    assert!(text(&o.stdout).contains("LLVM backend: 22 ("), "{}", text(&o.stdout));
}

/// An explicit choice that is not an installation is an error naming it
/// and what is wrong, before any work, whatever the action.
#[test]
fn explicit_invalid_is_an_error() {
    let d = scratch("invalid");
    fake(&d.join("good"), 22);
    std::fs::write(d.join("a.cpp"), "int x;\n").unwrap();
    script(&d.join("nolink/bin/opt"), "true");
    script(&d.join("nolink/bin/llc"), "true");
    std::fs::create_dir_all(d.join("nolink/lib/clang/22")).unwrap();
    let missing = d.join("missing");
    let o = kiln(&d, Some(&missing), &["-fsyntax-only", "a.cpp"]);
    assert_eq!(o.status.code(), Some(1));
    assert_eq!(
        text(&o.stderr),
        format!(
            "kiln++: error: '{}' (from KILN_LLVM_DIR) is not a usable LLVM installation: \
             no such directory\n",
            missing.display()
        )
    );
    // A bad flag does not fall back to a good KILN_LLVM_DIR.
    let flag = format!("--llvm-dir={}", d.join("nolink").display());
    let o = kiln(&d, Some(&d.join("good")), &["--version", &flag]);
    assert_eq!(o.status.code(), Some(1));
    assert_eq!(text(&o.stdout), "");
    assert_eq!(
        text(&o.stderr),
        format!(
            "kiln++: error: '{}' (from --llvm-dir) is not a usable LLVM installation: \
             no 'bin/clang++'\n",
            d.join("nolink").display()
        )
    );
}

/// LLVM 20 and 21 are refused with the major found and the supported set.
#[test]
fn unsupported_major_is_an_error() {
    let d = scratch("unsupported");
    fake(&d.join("llvm"), 21);
    let o = kiln(&d, Some(&d.join("llvm")), &["-c", "a.cpp"]);
    assert_eq!(o.status.code(), Some(1));
    assert_eq!(
        text(&o.stderr),
        format!(
            "kiln++: error: unsupported LLVM major 21 at '{}' (from KILN_LLVM_DIR); kiln \
             supports LLVM 19 and 22\n",
            d.join("llvm").display()
        )
    );
}

/// An empty `KILN_LLVM_DIR` is unset (the search runs; with `--help` no
/// LLVM is needed at all).
#[test]
fn empty_env_is_unset() {
    let d = scratch("empty");
    let o = kiln(&d, Some(Path::new("")), &["--help"]);
    assert_eq!(o.status.code(), Some(0));
    assert_eq!(text(&o.stderr), "");
}
