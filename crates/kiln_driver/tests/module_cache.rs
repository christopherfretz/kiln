//! WP-984: the toolchain's module-file cache for `import std;` and
//! `import std.compat;` (spec/modules.md §11.12, spec/driver.md "Module
//! cache") end to end through `kiln++`: a miss builds and stores, a hit
//! reuses, a changed input rebuilds, compiles racing on a miss all
//! succeed, an unwritable cache falls back to the in-process build, and
//! the cache can be moved and switched off.
//!
//! The tests use a miniature libc++ in a `--sysroot` (`std.cppm` and
//! `std.compat.cppm` over one header) so that a build takes milliseconds,
//! and every cache lives under the target directory (or, where advisory
//! locks do not work there, a temporary directory removed afterwards):
//! nothing here writes to the user's `~/.cache`. WP-1074 added the races
//! with more compiles, a killed one, and a held or stuck entry lock.
#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};

fn kilnpp() -> PathBuf {
    Path::new(env!("CARGO_BIN_EXE_kiln")).with_file_name("kiln++")
}

/// Whether an advisory lock (`flock`) on a fresh file in `dir` excludes
/// another open of it. It does not on some virtualised file systems (a
/// macOS directory mounted into a Linux container), where racing compiles
/// all build (spec/modules.md §11.12 "Concurrency"). There it sometimes
/// does for one file, so every one of several fresh files must exclude.
fn locks_exclude(dir: &Path) -> bool {
    (0..8).all(|i| {
        let probe = dir.join(format!("kiln-lock-probe-{}-{i}", std::process::id()));
        let open = || {
            std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(false)
                .open(&probe)
        };
        let excludes = match (open(), open()) {
            (Ok(a), Ok(b)) => {
                a.lock().is_ok() && matches!(b.try_lock(), Err(std::fs::TryLockError::WouldBlock))
            }
            _ => false,
        };
        let _ = std::fs::remove_file(&probe);
        excludes
    })
}

/// Where the tests' directories go: the target directory, unless locks do
/// not exclude there and they do in the system's temporary directory.
fn scratch_base() -> &'static Path {
    static BASE: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    BASE.get_or_init(|| {
        let target = PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
        let tmp = std::env::temp_dir();
        if !locks_exclude(&target) && locks_exclude(&tmp) { tmp } else { target }
    })
}

/// A fresh directory under [`scratch_base`].
fn scratch(name: &str) -> PathBuf {
    let d = scratch_base().join(format!("kiln-module-cache-{}-{name}", std::process::id()));
    if d.exists() {
        std::fs::remove_dir_all(&d).unwrap();
    }
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Removes a test's directory when it is not under the target directory.
fn done(d: &Path) {
    if !d.starts_with(env!("CARGO_TARGET_TMPDIR")) {
        std::fs::remove_dir_all(d).unwrap();
    }
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
    done(&d);
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
    done(&d);
}

/// The rule in the dependency file `f` in `dir`, unwrapped, as its
/// target and its prerequisites.
fn rule(dir: &Path, f: &str) -> Vec<String> {
    let text = std::fs::read_to_string(dir.join(f)).unwrap().replace(" \\\n  ", " ");
    let first = text.lines().next().unwrap_or_default().to_owned();
    first.split(' ').map(str::to_owned).collect()
}

/// WP-1051 (spec/driver.md "Dependency output", spec/modules.md §11.12):
/// a TU that imports `std` from the cache lists the module file it read
/// (absolute, the path `-v` reports) and not the headers inside it under
/// `-MMD`; under `-MD` the entry's stamped inputs follow as system files.
/// The file is the same on a miss and on a hit, and with either kind of
/// module file; `-fno-module-file-deps` drops the cache entirely.
#[test]
fn depfile_same_on_miss_and_hit() {
    let Some(d) = setup("deps") else { return };
    let dep = ["-fmodules-cache-path=cache", "-c", "u.cpp", "-o", "u.o"];
    let with = |extra: &[&str]| ok(&d, &[extra, &dep[..]].concat());
    // -MMD: the source and the cache entry, on a miss and on a hit.
    let miss = remarks(&with(&["-v", "-MMD", "-MF", "miss.d"]));
    assert!(miss[0].starts_with("building 'std' into '"), "{miss:?}");
    let hit = remarks(&with(&["-v", "-MMD", "-MF", "hit.d"]));
    let pcm = entries(&d.join("cache"))[0].join("std.pcm");
    assert_eq!(hit, [format!("using '{}' for 'std'", pcm.display())]);
    assert!(pcm.is_absolute(), "{}", pcm.display());
    let want = ["u.o:".to_owned(), "u.cpp".to_owned(), pcm.display().to_string()];
    assert_eq!(rule(&d, "miss.d"), want);
    assert_eq!(std::fs::read(d.join("miss.d")).unwrap(), std::fs::read(d.join("hit.d")).unwrap());
    // The other kind of module file reads the same entry.
    with(&["-fkiln-bmi-image", "-MMD", "-MF", "img.d"]);
    assert_eq!(std::fs::read(d.join("img.d")).unwrap(), std::fs::read(d.join("hit.d")).unwrap());
    // -MP: a phony rule for the entry too.
    with(&["-MMD", "-MP", "-MF", "mp.d"]);
    let mp = std::fs::read_to_string(d.join("mp.d")).unwrap();
    assert!(mp.ends_with(&format!("\n{}:\n", pcm.display())), "{mp}");
    // -fno-module-file-deps: neither the entry nor its inputs.
    with(&["-fno-module-file-deps", "-MD", "-MF", "no.d"]);
    assert_eq!(rule(&d, "no.d"), ["u.o:", "u.cpp"]);
    // -MD on a miss (a fresh cache) and on a hit: the entry, then the
    // stamped inputs (std.cppm first, then the header it includes).
    let md = ["-fmodules-cache-path=cache2", "-c", "u.cpp", "-o", "u.o", "-MD"];
    let miss = remarks(&ok(&d, &[&["-v", "-MF", "mdmiss.d"][..], &md].concat()));
    assert!(miss[0].starts_with("building 'std' into '"), "{miss:?}");
    let hit = remarks(&ok(&d, &[&["-v", "-MF", "mdhit.d"][..], &md].concat()));
    assert!(hit[0].starts_with("using"), "{hit:?}");
    assert_eq!(
        std::fs::read(d.join("mdmiss.d")).unwrap(),
        std::fs::read(d.join("mdhit.d")).unwrap()
    );
    let r = rule(&d, "mdmiss.d");
    let pcm2 = entries(&d.join("cache2"))[0].join("std.pcm");
    assert_eq!(r[..3], ["u.o:".to_owned(), "u.cpp".to_owned(), pcm2.display().to_string()]);
    assert_eq!(r.len(), 5, "{r:?}");
    assert!(r[3].ends_with("share/libc++/v1/std.cppm"), "{r:?}");
    assert!(r[4].ends_with("include/c++/v1/answer.h"), "{r:?}");
    // `import std.compat;` lists both entries it read.
    ok(&d, &["-fmodules-cache-path=cache", "-fsyntax-only", "-MMD", "-MF", "c.d", "c.cpp"]);
    let e = entries(&d.join("cache"))[0].clone();
    let c = rule(&d, "c.d");
    assert_eq!(c[..2], ["c.o:", "c.cpp"]);
    let mut listed = c[2..].to_vec();
    listed.sort();
    assert_eq!(
        listed,
        [e.join("std.compat.pcm").display().to_string(), e.join("std.pcm").display().to_string()]
    );
    done(&d);
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
    done(&d);
}

/// Starts `kiln++ -std=c++20 --sysroot=sys -stdlib=libc++
/// -fmodules-cache-path=cache <args>` in `dir`, with standard error piped.
fn spawn(dir: &Path, args: &[&str]) -> Child {
    Command::new(kilnpp())
        .args(["-std=c++20", "--sysroot=sys", "-stdlib=libc++", "-fmodules-cache-path=cache"])
        .args(args)
        .current_dir(dir)
        .env("XDG_CACHE_HOME", dir.join("xdg"))
        .env("HOME", dir.join("home"))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap()
}

/// `n` compiles of `c.cpp` started together on a cold cache, each with
/// `-v`; their standard errors, after checking that every one succeeded.
fn race(d: &Path, n: usize) -> Vec<String> {
    let children: Vec<_> =
        (0..n).map(|i| spawn(d, &["-v", "-c", "c.cpp", "-o", &format!("c{i}.o")])).collect();
    children
        .into_iter()
        .map(|c| {
            let o = c.wait_with_output().unwrap();
            let err = String::from_utf8_lossy(&o.stderr).into_owned();
            assert!(o.status.success(), "{err}");
            err
        })
        .collect()
}

/// After a race: one entry in `<d>/<cache>`, holding the two module files
/// and their stamps and nothing else (no temporary or lock file), which
/// the next compile uses.
fn check_settled(d: &Path, cache: &str) {
    let e = entries(&d.join(cache));
    assert_eq!(e.len(), 1, "{e:?}");
    assert_eq!(names(&e[0]), ["std.compat.pcm", "std.compat.stamp", "std.pcm", "std.stamp"]);
    let path = format!("-fmodules-cache-path={cache}");
    let r = remarks(&ok(d, &["-v", &path, "-fsyntax-only", "c.cpp"]));
    assert!(r.iter().all(|l| l.starts_with("using")), "{r:?}");
}

#[test]
fn racing_misses_all_succeed() {
    let Some(d) = setup("race") else { return };
    race(&d, 4);
    check_settled(&d, "cache");
    done(&d);
}

/// WP-1074: before the entry lock, every racer built and renamed both
/// files, and the module file and the stamp left last could come from
/// different racers (about 1 in 12 races of 16), so the next compile
/// missed. Now exactly one racer builds; the others wait and use it.
#[test]
fn many_racing_misses_build_once() {
    let Some(d) = setup("race16") else { return };
    let errs = race(&d, 16);
    let built: usize = errs
        .iter()
        .map(|e| remarks(e).iter().filter(|l| l.starts_with("building 'std.compat' into")).count())
        .sum();
    // Where locks do not exclude, every racer may build.
    if locks_exclude(scratch_base()) {
        assert_eq!(built, 1, "{errs:#?}");
    } else {
        assert!(built >= 1, "{errs:#?}");
    }
    check_settled(&d, "cache");
    done(&d);
}

/// WP-1074: a racer killed (SIGKILL) at some point of its compile, maybe
/// while it holds the lock and builds, neither fails the others nor
/// leaves a file behind.
#[test]
fn killed_racer_leaves_no_trace() {
    let Some(d) = setup("killed") else { return };
    for (round, delay) in [0u64, 15, 40].into_iter().enumerate() {
        let cache = format!("-fmodules-cache-path=cache{round}");
        let mut victim = spawn(&d, &[&cache, "-c", "c.cpp", "-o", "k.o"]);
        let others: Vec<_> =
            (0..5).map(|i| spawn(&d, &[&cache, "-c", "c.cpp", "-o", &format!("c{i}.o")])).collect();
        std::thread::sleep(std::time::Duration::from_millis(delay));
        let _ = victim.kill();
        let _ = victim.wait();
        for c in others {
            let o = c.wait_with_output().unwrap();
            assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
        }
        // The next compile finishes whatever the victim left, if anything.
        ok(&d, &[&cache, "-fsyntax-only", "c.cpp"]);
        check_settled(&d, &format!("cache{round}"));
    }
    done(&d);
}

/// The entry directory `d`'s cache settles on for `c.cpp`, made a miss
/// again: both stamps removed.
fn emptied_entry(d: &Path) -> PathBuf {
    ok(d, &["-fmodules-cache-path=cache", "-fsyntax-only", "c.cpp"]);
    let e = entries(&d.join("cache")).remove(0);
    std::fs::remove_file(e.join("std.stamp")).unwrap();
    std::fs::remove_file(e.join("std.compat.stamp")).unwrap();
    e
}

/// Holds `<entry>/std.compat.lock` as a compile building `std.compat`
/// would.
fn hold_lock(entry: &Path) -> std::fs::File {
    let f = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(entry.join("std.compat.lock"))
        .unwrap();
    f.lock().unwrap();
    f
}

/// WP-1074: a miss waits while another compile holds the entry's lock;
/// when the holder goes away (here it closes the lock file; a killed one
/// is released by the kernel the same way) it builds, clearing the
/// temporary files the holder left, and removes the lock file.
#[test]
fn miss_waits_for_the_lock_holder() {
    let Some(d) = setup("held") else { return };
    if !locks_exclude(scratch_base()) {
        eprintln!("advisory locks do not exclude here; skipping");
        return done(&d);
    }
    let e = emptied_entry(&d);
    let left = ["std.compat.pcm.tmp.4242", "std.pcm.tmp.4242", "std.stamp.tmp.4242.ThreadId(1).7"];
    for f in left {
        std::fs::write(e.join(f), "partial").unwrap();
    }
    let held = hold_lock(&e);
    let mut child = spawn(&d, &["-v", "-fsyntax-only", "c.cpp"]);
    let mut err = std::io::BufReader::new(child.stderr.take().unwrap());
    let mut text = String::new();
    // Read up to the wait remark: the child now waits on the lock.
    loop {
        let mut line = String::new();
        assert!(std::io::BufRead::read_line(&mut err, &mut line).unwrap() > 0, "{text}");
        text.push_str(&line);
        if line.contains("waiting for another compile building 'std.compat'") {
            break;
        }
    }
    assert!(child.try_wait().unwrap().is_none(), "{text}");
    drop(held);
    std::io::Read::read_to_string(&mut err, &mut text).unwrap();
    assert!(child.wait().unwrap().success(), "{text}");
    let r = remarks(&text);
    assert!(r[1].starts_with("building 'std.compat' into"), "{r:?}");
    check_settled(&d, "cache");
    done(&d);
}

/// WP-1074: a holder that never finishes delays a miss by the wait limit
/// (`KILN_MODULE_CACHE_WAIT_MS`, here 200 ms) at most; the miss then
/// builds the module from its source and succeeds.
#[test]
fn stuck_holder_falls_back() {
    let Some(d) = setup("stuck") else { return };
    if !locks_exclude(scratch_base()) {
        eprintln!("advisory locks do not exclude here; skipping");
        return done(&d);
    }
    let e = emptied_entry(&d);
    let held = hold_lock(&e);
    let o = Command::new(kilnpp())
        .args(["-std=c++20", "--sysroot=sys", "-stdlib=libc++", "-fmodules-cache-path=cache"])
        .args(["-v", "-c", "c.cpp", "-o", "c.o"])
        .current_dir(&d)
        .env("KILN_MODULE_CACHE_WAIT_MS", "200")
        .stdin(Stdio::null())
        .output()
        .unwrap();
    let err = String::from_utf8_lossy(&o.stderr);
    assert!(o.status.success(), "{err}");
    let r = remarks(&err);
    assert!(r[0].starts_with("waiting for another compile building 'std.compat'"), "{r:?}");
    assert!(r[1].starts_with("gave up waiting for '"), "{r:?}");
    assert!(r[1].ends_with("std.compat.lock'; building 'std.compat' from its source"), "{r:?}");
    // Built from its source, `std.compat` imported `std` through the
    // cache; its own entry is untouched, still a miss and still held.
    assert!(r[2].starts_with("building 'std' into"), "{r:?}");
    assert_eq!(names(&e), ["std.compat.lock", "std.compat.pcm", "std.pcm", "std.stamp"]);
    drop(held);
    ok(&d, &["-fmodules-cache-path=cache", "-fsyntax-only", "c.cpp"]);
    check_settled(&d, "cache");
    done(&d);
}

/// WP-1074: a lock file a killed compile left in a valid entry is removed
/// by the next hit.
#[test]
fn hit_removes_a_stale_lock_file() {
    let Some(d) = setup("stale-lock") else { return };
    ok(&d, &["-fmodules-cache-path=cache", "-fsyntax-only", "c.cpp"]);
    let e = entries(&d.join("cache")).remove(0);
    std::fs::write(e.join("std.compat.lock"), "").unwrap();
    std::fs::write(e.join("std.lock"), "").unwrap();
    let r = remarks(&ok(&d, &["-v", "-fmodules-cache-path=cache", "-fsyntax-only", "c.cpp"]));
    assert!(r.iter().all(|l| l.starts_with("using")), "{r:?}");
    check_settled(&d, "cache");
    done(&d);
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
    done(&d);
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
    done(&d);
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
    done(&d);
}
