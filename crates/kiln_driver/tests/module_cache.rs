//! WP-984: the toolchain's module-file cache for `import std;` and
//! `import std.compat;` (spec/modules.md §11.11, spec/driver.md "Module
//! cache") end to end through `kiln++`: a miss builds and stores, a hit
//! reuses, a changed input rebuilds, compiles racing on a miss all
//! succeed, an unwritable cache falls back to the in-process build, and
//! the cache can be moved and switched off.
//!
//! The tests use a miniature libc++ in a `--sysroot` (`std.cppm` and
//! `std.compat.cppm` over one header) so that a build takes milliseconds,
//! and every cache lives under the target directory: nothing here writes
//! to the user's `~/.cache`.
#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

fn kilnpp() -> PathBuf {
    Path::new(env!("CARGO_BIN_EXE_kiln")).with_file_name("kiln++")
}

/// A fresh directory under the target directory.
fn scratch(name: &str) -> PathBuf {
    let d = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("kiln-module-cache-{}-{name}", std::process::id()));
    if d.exists() {
        std::fs::remove_dir_all(&d).unwrap();
    }
    std::fs::create_dir_all(&d).unwrap();
    d
}

const STD: &str = "module;\n#include <answer.h>\nexport module std;\n\
                   export namespace std { using ::answer; }\n";
const COMPAT: &str = "module;\n#include <answer.h>\nexport module std.compat;\n\
                      export import std;\nexport using ::answer;\n";
const USE_STD: &str = "import std;\nint main() { return std::answer() == 42 ? 0 : 1; }\n";
const USE_COMPAT: &str =
    "import std.compat;\nint main() { return std::answer() + answer() == 84 ? 0 : 1; }\n";

/// A test directory: a miniature libc++ under `sys/` (headers, module
/// sources and manifest where kiln looks for them), the importers
/// `u.cpp` and `c.cpp`, and the cache root `cache/` (not created).
/// `None` when this toolchain does not take libc++ from the sysroot (an
/// LLVM directory with its own libc++ comes first), so the miniature
/// cannot stand in for it.
fn setup(name: &str) -> Option<PathBuf> {
    let d = scratch(name);
    let sys = d.join("sys/usr");
    std::fs::create_dir_all(sys.join("include/c++/v1")).unwrap();
    std::fs::create_dir_all(sys.join("share/libc++/v1")).unwrap();
    std::fs::create_dir_all(sys.join("lib")).unwrap();
    std::fs::write(sys.join("include/c++/v1/answer.h"), "inline int answer() { return 42; }\n")
        .unwrap();
    std::fs::write(sys.join("share/libc++/v1/std.cppm"), STD).unwrap();
    std::fs::write(sys.join("share/libc++/v1/std.compat.cppm"), COMPAT).unwrap();
    std::fs::write(sys.join("lib/libc++.modules.json"), "{}\n").unwrap();
    std::fs::write(d.join("u.cpp"), USE_STD).unwrap();
    std::fs::write(d.join("c.cpp"), USE_COMPAT).unwrap();
    let o = Command::new(kilnpp())
        .args(["--sysroot=sys", "-stdlib=libc++", "-print-file-name=libc++.modules.json"])
        .current_dir(&d)
        .output()
        .unwrap();
    let printed = String::from_utf8_lossy(&o.stdout).trim().to_owned();
    if Path::new(&printed) != d.join("sys/usr/lib/libc++.modules.json") {
        eprintln!("libc++ does not come from the sysroot here ({printed}); skipping");
        return None;
    }
    Some(d)
}

struct Out {
    code: i32,
    stderr: String,
}

/// `kiln++ -std=c++20 --sysroot=sys -stdlib=libc++ <args>` in `dir`, with
/// `XDG_CACHE_HOME` and `HOME` under `dir` (so a test that names no cache
/// cannot reach the real one).
fn run(dir: &Path, args: &[&str]) -> Out {
    let o: Output = Command::new(kilnpp())
        .args(["-std=c++20", "--sysroot=sys", "-stdlib=libc++"])
        .args(args)
        .current_dir(dir)
        .env("XDG_CACHE_HOME", dir.join("xdg"))
        .env("HOME", dir.join("home"))
        .stdin(Stdio::null())
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&o.stderr).into_owned();
    assert!(!stderr.contains("panicked"), "kiln++ {args:?} panicked:\n{stderr}");
    Out { code: o.status.code().unwrap_or(-1), stderr }
}

fn ok(dir: &Path, args: &[&str]) -> String {
    let o = run(dir, args);
    assert_eq!(o.code, 0, "kiln++ {args:?}:\n{}", o.stderr);
    o.stderr
}

/// The `module cache:` lines of `-v` output.
fn remarks(stderr: &str) -> Vec<String> {
    stderr
        .lines()
        .filter_map(|l| l.strip_prefix("kiln++: module cache: "))
        .map(str::to_owned)
        .collect()
}

/// The entry directories under `root`.
fn entries(root: &Path) -> Vec<PathBuf> {
    let Ok(rd) = std::fs::read_dir(root) else { return Vec::new() };
    let mut v: Vec<PathBuf> = rd.map(|e| e.unwrap().path()).collect();
    v.sort();
    v
}

/// The file names in `dir`, sorted.
fn names(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    v.sort();
    v
}

#[test]
fn miss_builds_then_hit_reuses() {
    let Some(d) = setup("hit") else { return };
    let r = remarks(&ok(&d, &["-v", "-fmodules-cache-path=cache", "-c", "u.cpp", "-o", "u.o"]));
    assert_eq!(r.len(), 1, "{r:?}");
    assert!(r[0].starts_with("building 'std' into '"), "{r:?}");
    let e = entries(&d.join("cache"));
    assert_eq!(e.len(), 1, "{e:?}");
    assert_eq!(names(&e[0]), ["std.pcm", "std.stamp"]);
    assert!(std::fs::read(e[0].join("std.pcm")).unwrap().starts_with(b"KILNBMI\0"));
    let r = remarks(&ok(&d, &["-v", "-fmodules-cache-path=cache", "-c", "u.cpp", "-o", "u.o"]));
    assert_eq!(r, [format!("using '{}' for 'std'", e[0].join("std.pcm").display())]);
    // Without `-v` the cache says nothing.
    assert_eq!(ok(&d, &["-fmodules-cache-path=cache", "-fsyntax-only", "u.cpp"]), "");
    // The object is written (the miniature sysroot has nothing to link).
    std::fs::remove_file(d.join("u.o")).unwrap();
    ok(&d, &["-fmodules-cache-path=cache", "-c", "u.cpp", "-o", "u.o"]);
    assert!(d.join("u.o").is_file());
}

#[test]
fn std_compat_caches_both_modules() {
    let Some(d) = setup("compat") else { return };
    let r = remarks(&ok(&d, &["-v", "-fmodules-cache-path=cache", "-fsyntax-only", "c.cpp"]));
    assert!(r[0].starts_with("building 'std.compat' into"), "{r:?}");
    let e = entries(&d.join("cache"));
    assert_eq!(e.len(), 1, "{e:?}");
    // Building std.compat built std through the cache as well.
    assert_eq!(names(&e[0]), ["std.compat.pcm", "std.compat.stamp", "std.pcm", "std.stamp"]);
    let r = remarks(&ok(&d, &["-v", "-fmodules-cache-path=cache", "-fsyntax-only", "c.cpp"]));
    assert!(r.iter().all(|l| l.starts_with("using")), "{r:?}");
    assert!(r.iter().any(|l| l.ends_with("for 'std.compat'")), "{r:?}");
    // `import std;` then hits the entry std.compat filled.
    let r = remarks(&ok(&d, &["-v", "-fmodules-cache-path=cache", "-fsyntax-only", "u.cpp"]));
    assert_eq!(r.len(), 1, "{r:?}");
    assert!(r[0].starts_with("using"), "{r:?}");
}

#[test]
fn changed_inputs_rebuild() {
    let Some(d) = setup("stale") else { return };
    let args = ["-v", "-fmodules-cache-path=cache", "-fsyntax-only", "u.cpp"];
    ok(&d, &args);
    // A header the module includes, edited in place: same key, stale stamp.
    let h = d.join("sys/usr/include/c++/v1/answer.h");
    std::fs::write(&h, "inline int answer() { return 40 + 2; }\n").unwrap();
    let r = remarks(&ok(&d, &args));
    assert!(r[0].starts_with("building 'std'"), "{r:?}");
    assert_eq!(entries(&d.join("cache")).len(), 1);
    // A damaged module file fails its stamp and is rebuilt.
    let e = entries(&d.join("cache"));
    std::fs::write(e[0].join("std.pcm"), b"KILNBMI\0").unwrap();
    let r = remarks(&ok(&d, &args));
    assert!(r[0].starts_with("building 'std'"), "{r:?}");
    assert!(remarks(&ok(&d, &args))[0].starts_with("using"));
    // A different std.cppm, or a different configuration, is another key.
    std::fs::write(d.join("sys/usr/share/libc++/v1/std.cppm"), format!("{STD}// v2\n")).unwrap();
    ok(&d, &args);
    assert_eq!(entries(&d.join("cache")).len(), 2);
    ok(&d, &["-DX=1", "-fmodules-cache-path=cache", "-fsyntax-only", "u.cpp"]);
    ok(&d, &["-fno-exceptions", "-fmodules-cache-path=cache", "-fsyntax-only", "u.cpp"]);
    assert_eq!(entries(&d.join("cache")).len(), 4);
}

#[test]
fn racing_misses_all_succeed() {
    let Some(d) = setup("race") else { return };
    let children: Vec<_> = (0..4)
        .map(|i| {
            Command::new(kilnpp())
                .args(["-std=c++20", "--sysroot=sys", "-stdlib=libc++"])
                .args(["-fmodules-cache-path=cache", "-c", "c.cpp", "-o"])
                .arg(format!("c{i}.o"))
                .current_dir(&d)
                .stdin(Stdio::null())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap()
        })
        .collect();
    for c in children {
        let o = c.wait_with_output().unwrap();
        assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    }
    let e = entries(&d.join("cache"));
    assert_eq!(e.len(), 1, "{e:?}");
    // No temporary file is left behind, and the entry is valid.
    assert_eq!(names(&e[0]), ["std.compat.pcm", "std.compat.stamp", "std.pcm", "std.stamp"]);
    let r = remarks(&ok(&d, &["-v", "-fmodules-cache-path=cache", "-fsyntax-only", "c.cpp"]));
    assert!(r.iter().all(|l| l.starts_with("using")), "{r:?}");
}

#[test]
fn unwritable_cache_falls_back() {
    let Some(d) = setup("unwritable") else { return };
    // A cache root below a regular file cannot be created (permissions
    // would not stop root, which CI may run as).
    std::fs::write(d.join("file"), "").unwrap();
    let r = remarks(&ok(&d, &["-v", "-fmodules-cache-path=file/cache", "-c", "u.cpp"]));
    assert_eq!(r.len(), 1, "{r:?}");
    assert!(r[0].starts_with("cannot create '"), "{r:?}");
    assert!(r[0].ends_with("; building 'std' from its source"), "{r:?}");
    // Silently without `-v`.
    assert_eq!(ok(&d, &["-fmodules-cache-path=file/cache", "-c", "u.cpp"]), "");
}

#[test]
fn location_and_switches() {
    let Some(d) = setup("where") else { return };
    // `$XDG_CACHE_HOME/kiln/modules` by default.
    ok(&d, &["-fsyntax-only", "u.cpp"]);
    assert_eq!(entries(&d.join("xdg/kiln/modules")).len(), 1);
    // Then `$HOME/.cache/kiln/modules`.
    let o = Command::new(kilnpp())
        .args(["-std=c++20", "--sysroot=sys", "-stdlib=libc++", "-fsyntax-only", "u.cpp"])
        .current_dir(&d)
        .env_remove("XDG_CACHE_HOME")
        .env("HOME", d.join("home"))
        .output()
        .unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    assert_eq!(entries(&d.join("home/.cache/kiln/modules")).len(), 1);
    // Off: `-fno-kiln-module-cache`, or an empty `-fmodules-cache-path=`.
    let r = remarks(&ok(&d, &["-v", "-fno-kiln-module-cache", "-fsyntax-only", "u.cpp"]));
    assert!(r.is_empty(), "{r:?}");
    let r = remarks(&ok(&d, &["-v", "-fmodules-cache-path=", "-fsyntax-only", "u.cpp"]));
    assert!(r.is_empty(), "{r:?}");
    ok(&d, &["-fno-kiln-module-cache", "-fmodules-cache-path=off", "-fsyntax-only", "u.cpp"]);
    assert!(!d.join("off").exists());
    // A module file given on the command line wins; no cache is consulted.
    ok(
        &d,
        &["--precompile", "-x", "c++-module", "sys/usr/share/libc++/v1/std.cppm", "-o", "s.pcm"],
    );
    let r = remarks(&ok(
        &d,
        &["-v", "-fmodules-cache-path=given", "-fmodule-file=std=s.pcm", "-fsyntax-only", "u.cpp"],
    ));
    assert!(r.is_empty(), "{r:?}");
    assert!(!d.join("given").exists());
}

#[test]
fn print_file_name_finds_the_manifest() {
    let Some(d) = setup("json") else { return };
    // setup() checked the miniature; the real toolchain's manifest, when
    // there is one, names a std.cppm that exists.
    let o = Command::new(kilnpp())
        .args(["-stdlib=libc++", "-print-file-name=libc++.modules.json"])
        .current_dir(&d)
        .output()
        .unwrap();
    let printed = PathBuf::from(String::from_utf8_lossy(&o.stdout).trim());
    if printed.is_absolute() {
        let text = std::fs::read_to_string(&printed).unwrap();
        let at = text.find("\"source-path\"").expect("a source-path entry");
        let rest = &text[at + "\"source-path\"".len()..];
        let rel = rest.split('"').nth(1).unwrap();
        assert!(printed.parent().unwrap().join(rel).is_file(), "{rel} from {}", printed.display());
    }
}
