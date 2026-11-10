//! The note after a missing `#include <meta>` at C++26 (WP-1210; spec/pp.md
//! "Missing `<meta>`", [headers]/[meta.syn]): `<meta>` (P2996) ships only
//! with GCC 16.1's libstdc++, so the `'meta' file not found` fatal error
//! names the library kiln selected and, when `g++-16` (then `g++`) on
//! `PATH` reports a GCC 16 installation, its `--gcc-toolchain` prefix. The
//! error itself is unchanged; the `PATH` probe runs only on this error
//! path. A fake `g++-16` script on a temporary `PATH` stands in for a real
//! GCC 16; kiln++ runs as a child so its `PATH` can differ.
#![forbid(unsafe_code)]
#![cfg(unix)]

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

fn kilnpp() -> PathBuf {
    Path::new(env!("CARGO_BIN_EXE_kiln")).with_file_name("kiln++")
}

struct Tree {
    root: PathBuf,
    /// A fake GCC 14 installation (`.../lib/gcc/<triple>/14`).
    gcc14: String,
    /// The prefix of a fake GCC 16.1 installation.
    gcc16_prefix: PathBuf,
    /// `bin/` with a fake `g++-16` reporting it.
    bin: PathBuf,
    /// Created by the fake `g++-16` when it runs.
    marker: PathBuf,
}

fn tree(name: &str) -> Tree {
    let root = std::env::temp_dir().join(format!("kiln-meta-note-{}-{name}", std::process::id()));
    let gcc14 = root.join("old/lib/gcc/aarch64-linux-gnu/14");
    let gcc16 = root.join("new/lib/gcc/aarch64-linux-gnu/16.1.0");
    for d in [&gcc14, &gcc16] {
        std::fs::create_dir_all(d).unwrap();
        std::fs::write(d.join("crtbegin.o"), "").unwrap();
    }
    std::fs::create_dir_all(root.join("new/bin")).unwrap();
    let bin = root.join("path");
    std::fs::create_dir_all(&bin).unwrap();
    let marker = root.join("probed");
    let script = bin.join("g++-16");
    std::fs::write(
        &script,
        format!(
            "#!/bin/sh\ntouch '{}'\necho 'install: {}/new/bin/../lib/gcc/aarch64-linux-gnu/16.1.0/'\n",
            marker.display(),
            root.display()
        ),
    )
    .unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    std::fs::write(root.join("meta.cpp"), "#include <meta>\nint main() {}\n").unwrap();
    let gcc16_prefix = std::fs::canonicalize(root.join("new")).unwrap();
    Tree { gcc14: gcc14.display().to_string(), root, gcc16_prefix, bin, marker }
}

/// Runs kiln++ in the tree with `bin` first on `PATH` (or not at all).
fn run(t: &Tree, probe: bool, args: &[&str]) -> (Option<i32>, String) {
    let path = std::env::var("PATH").unwrap_or_default();
    let path = if probe { format!("{}:{path}", t.bin.display()) } else { path };
    let o = Command::new(kilnpp())
        .args(["-fsyntax-only"])
        .args(args)
        .env("PATH", path)
        .current_dir(&t.root)
        .output()
        .unwrap();
    (o.status.code(), String::from_utf8_lossy(&o.stderr).into_owned())
}

const ERROR: &str = "meta.cpp:1:10: fatal error: 'meta' file not found\n";

fn note(stderr: &str) -> Option<&str> {
    stderr.lines().find(|l| l.starts_with("note: "))
}

#[test]
fn meta_note_names_the_selected_gcc_and_the_path_gcc16() {
    let t = tree("gcc");
    let install = format!("--gcc-install-dir={}", t.gcc14);
    let (code, err) = run(&t, true, &["-std=c++26", &install, "meta.cpp"]);
    assert_eq!(code, Some(1), "{err}");
    assert!(err.starts_with(ERROR), "{err}");
    assert_eq!(
        note(&err),
        Some(
            format!(
                "note: '<meta>' is provided by GCC 16.1's libstdc++; kiln is using the GCC \
                 installation at '{}' (version 14); pass --gcc-toolchain=<prefix of a GCC 16.1 \
                 installation>; a GCC 16 installation was found at '{}' (from g++-16 on PATH)",
                t.gcc14,
                t.gcc16_prefix.display()
            )
            .as_str()
        ),
        "{err}"
    );
    assert!(t.marker.is_file());

    // `-stdlib=libc++` (and `-std=gnu++2c`).
    let (code, err) = run(&t, true, &["-std=gnu++2c", "-stdlib=libc++", &install, "meta.cpp"]);
    assert_eq!(code, Some(1), "{err}");
    assert!(err.starts_with(ERROR), "{err}");
    assert_eq!(
        note(&err),
        Some(
            format!(
                "note: libc++ does not provide '<meta>'; it is provided by GCC 16.1's \
                 libstdc++: pass -stdlib=libstdc++ --gcc-toolchain=<prefix of a GCC 16.1 \
                 installation>; a GCC 16 installation was found at '{}' (from g++-16 on PATH)",
                t.gcc16_prefix.display()
            )
            .as_str()
        ),
        "{err}"
    );

    // No GCC installation at all (an empty `--gcc-toolchain` prefix).
    let empty = format!("--gcc-toolchain={}/none", t.root.display());
    let (_, err) = run(&t, true, &["-std=c++2c", &empty, "meta.cpp"]);
    assert!(err.starts_with(ERROR), "{err}");
    assert!(
        note(&err).is_some_and(|n| n.starts_with(
            "note: '<meta>' is provided by GCC 16.1's libstdc++, but kiln found no GCC \
             installation; pass --gcc-toolchain=<prefix of a GCC 16.1 installation>"
        )),
        "{err}"
    );
    std::fs::remove_dir_all(&t.root).unwrap();
}

#[test]
fn meta_note_only_at_cxx26_and_only_on_the_error_path() {
    let t = tree("std");
    let install = format!("--gcc-install-dir={}", t.gcc14);
    // Below C++26: no reflection, no note, no probe.
    for std in ["-std=c++20", "-std=c++23"] {
        let (code, err) = run(&t, true, &[std, &install, "meta.cpp"]);
        assert_eq!(
            (code, err.as_str()),
            (
                Some(1),
                &*format!(
                    "{ERROR}    1 | #include <meta>\n      |          ^\n1 error generated.\n"
                )
            )
        );
    }
    // `#include "meta"` (quoted) is not the standard header.
    std::fs::write(t.root.join("quoted.cpp"), "#include \"meta\"\n").unwrap();
    let (_, err) = run(&t, true, &["-std=c++26", &install, "quoted.cpp"]);
    assert!(note(&err).is_none(), "{err}");
    assert!(!t.marker.exists());
    // Found: the compile succeeds and the probe never runs.
    std::fs::create_dir_all(t.root.join("inc")).unwrap();
    std::fs::write(t.root.join("inc/meta"), "").unwrap();
    let (code, err) = run(&t, true, &["-std=c++26", &install, "-Iinc", "meta.cpp"]);
    assert_eq!((code, err.as_str()), (Some(0), ""));
    assert!(!t.marker.exists());
    // Without a GCC 16 on `PATH` the note has no suffix (unless the host's
    // own `g++-16` / `g++` is a GCC 16).
    let (_, err) = run(&t, false, &["-std=c++26", &install, "meta.cpp"]);
    assert!(
        note(&err).is_some_and(|n| n.contains(&format!("'{}' (version 14)", t.gcc14))),
        "{err}"
    );
    std::fs::remove_dir_all(&t.root).unwrap();
}
