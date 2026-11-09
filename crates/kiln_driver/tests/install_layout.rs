//! The relocatable install layout's resource-directory discovery
//! (spec/driver.md "Installation layout" and "Toolchain", WP-927): a
//! `kiln` binary placed in `<prefix>/bin` finds `<prefix>/lib/kiln/<major>`
//! when run directly, through the `kiln++` symlink, through a symlink to
//! that elsewhere and through `PATH`; `$KILN_RESOURCE_DIR` and
//! `-resource-dir` still take precedence.
#![forbid(unsafe_code)]
#![cfg(unix)]

use std::path::{Path, PathBuf};
use std::process::Command;

/// A fake install tree under the test's temporary directory: the built
/// `kiln` hard-linked (else copied) to `bin/kiln`, `bin/kiln++ -> kiln`,
/// and an empty resource directory with `predefines/` and `include/`.
/// Returns the prefix, canonicalized.
fn install(name: &str) -> PathBuf {
    let prefix = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&prefix);
    let bin = prefix.join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    let exe = Path::new(env!("CARGO_BIN_EXE_kiln"));
    if std::fs::hard_link(exe, bin.join("kiln")).is_err() {
        std::fs::copy(exe, bin.join("kiln")).unwrap();
    }
    std::os::unix::fs::symlink("kiln", bin.join("kiln++")).unwrap();
    let res = resource_dir(&prefix);
    std::fs::create_dir_all(res.join("predefines")).unwrap();
    std::fs::create_dir_all(res.join("include")).unwrap();
    std::fs::canonicalize(&prefix).unwrap()
}

fn resource_dir(prefix: &Path) -> PathBuf {
    prefix.join("lib").join("kiln").join(kiln_driver::version::MAJOR.to_string())
}

/// `<prog> -print-resource-dir <extra>` with `KILN_RESOURCE_DIR` removed
/// and `dir` as the working directory.
fn print_resource_dir(prog: &Path, dir: &Path, extra: &[&str]) -> String {
    let o = Command::new(prog)
        .arg("-print-resource-dir")
        .args(extra)
        .env_remove("KILN_RESOURCE_DIR")
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    String::from_utf8(o.stdout).unwrap().trim_end().to_owned()
}

#[test]
fn installed_binary_finds_its_resource_directory() {
    let prefix = install("install-layout");
    let want = resource_dir(&prefix).display().to_string();
    let bin = prefix.join("bin");
    // Directly, through the kiln++ symlink, and through a symlink to that
    // symlink outside the tree.
    let elsewhere = prefix.parent().unwrap().join("install-layout-link");
    let _ = std::fs::remove_file(&elsewhere);
    std::os::unix::fs::symlink(bin.join("kiln++"), &elsewhere).unwrap();
    for prog in [bin.join("kiln"), bin.join("kiln++"), elsewhere.clone()] {
        assert_eq!(print_resource_dir(&prog, &prefix, &[]), want, "{}", prog.display());
    }
    // Through PATH, from another directory.
    let o = Command::new("kiln++")
        .arg("-print-resource-dir")
        .env_remove("KILN_RESOURCE_DIR")
        .env("PATH", &bin)
        .current_dir(prefix.parent().unwrap())
        .output()
        .unwrap();
    assert_eq!(String::from_utf8_lossy(&o.stdout).trim_end(), want);
    std::fs::remove_file(&elsewhere).unwrap();
}

#[test]
fn environment_and_flag_take_precedence() {
    let prefix = install("install-layout-precedence");
    let kiln = prefix.join("bin").join("kiln");
    let o = Command::new(&kiln)
        .arg("-print-resource-dir")
        .env("KILN_RESOURCE_DIR", "/from/env")
        .output()
        .unwrap();
    assert_eq!(String::from_utf8_lossy(&o.stdout).trim_end(), "/from/env");
    let o = Command::new(&kiln)
        .args(["-print-resource-dir", "-resource-dir", "/from/flag"])
        .env("KILN_RESOURCE_DIR", "/from/env")
        .output()
        .unwrap();
    assert_eq!(String::from_utf8_lossy(&o.stdout).trim_end(), "/from/flag");
    // An empty KILN_RESOURCE_DIR is unset.
    let o = Command::new(&kiln)
        .arg("-print-resource-dir")
        .env("KILN_RESOURCE_DIR", "")
        .output()
        .unwrap();
    assert_eq!(
        String::from_utf8_lossy(&o.stdout).trim_end(),
        resource_dir(&prefix).display().to_string()
    );
}

/// Without `lib/kiln/<major>/predefines`, a source build falls back to the
/// checkout's `resource/` (spec/driver.md "Toolchain", step 5).
#[test]
fn incomplete_tree_falls_back_to_the_source_tree() {
    let prefix = install("install-layout-incomplete");
    std::fs::remove_dir(resource_dir(&prefix).join("predefines")).unwrap();
    let got = print_resource_dir(&prefix.join("bin").join("kiln"), &prefix, &[]);
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resource");
    assert_eq!(std::fs::canonicalize(&got).unwrap(), std::fs::canonicalize(src).unwrap(), "{got}");
}
