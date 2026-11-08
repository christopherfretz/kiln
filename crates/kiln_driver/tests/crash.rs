//! The internal-compiler-error path end to end (spec/driver.md "Internal
//! compiler errors"; WP-911): the banner, the exit codes, the crash
//! reproducer and the flags and environment that steer them, on the real
//! binary crashing on purpose (`-fkiln-crash-for-testing=<phase>`).
#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

/// The `kiln++` beside the `kiln` binary under test (spec/driver.md
/// "Driver modes").
fn kilnpp() -> PathBuf {
    Path::new(env!("CARGO_BIN_EXE_kiln")).with_file_name("kiln++")
}

/// A fresh directory for one test, holding `a.cpp`.
fn workdir(name: &str) -> PathBuf {
    let d = Path::new(env!("CARGO_TARGET_TMPDIR")).join("crash-tests").join(name);
    if d.exists() {
        std::fs::remove_dir_all(&d).unwrap();
    }
    std::fs::create_dir_all(&d).unwrap();
    std::fs::write(d.join("a.cpp"), "#define N 1\nint first = N;\nconstexpr int two = 2;\n")
        .unwrap();
    d
}

/// Runs `program args` in `cwd` with a clean crash environment (no
/// backtrace variables, `TMPDIR` = `tmp`).
fn run(program: &Path, cwd: &Path, tmp: &Path, args: &[&str], env: &[(&str, &str)]) -> Output {
    let mut c = Command::new(program);
    c.current_dir(cwd)
        .args(args)
        .env_remove("RUST_BACKTRACE")
        .env_remove("KILN_BACKTRACE")
        .env("TMPDIR", tmp)
        .stdin(Stdio::null());
    for (k, v) in env {
        c.env(k, v);
    }
    c.output().unwrap()
}

fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

/// The files of `dir`, sorted (none when it does not exist).
fn files(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|rd| rd.filter_map(|e| e.ok().map(|e| e.path())).collect())
        .unwrap_or_default();
    v.sort();
    v
}

/// The paths the banner lists on its `diagnostic msg:` lines.
fn listed(err: &str) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = err
        .lines()
        .filter_map(|l| l.split_once("note: diagnostic msg: ").map(|(_, p)| PathBuf::from(p)))
        .filter(|p| p.is_absolute())
        .collect();
    v.sort();
    v
}

#[test]
fn banner_and_reproducer() {
    let d = workdir("banner");
    let crash = d.join("crash");
    let dir_flag = format!("-fcrash-diagnostics-dir={}", crash.display());
    let o = run(
        &kilnpp(),
        &d,
        &d,
        &["-fsyntax-only", "-DFOO=a b", "-fkiln-crash-for-testing=parse", &dir_flag, "a.cpp"],
        &[],
    );
    let err = stderr(&o);
    // clang 19 exits 1 once it has written its reproducer.
    assert_eq!(o.status.code(), Some(1), "{err}");
    let lines: Vec<&str> = err.lines().collect();
    assert_eq!(
        lines[0],
        "kiln++: error: internal compiler error: crash requested by -fkiln-crash-for-testing=parse"
    );
    assert_eq!(lines[1], "kiln++: note: while compiling 'a.cpp' (phase: parse, at a.cpp:2:1)");
    assert!(lines[2].starts_with("kiln++: note: in compiler source crates/"), "{err}");
    assert!(lines[2].ends_with("(thread 'kiln')"), "{err}");
    assert!(lines[3].starts_with(&format!("kiln {} (", env!("CARGO_PKG_VERSION"))), "{err}");
    assert_eq!(
        lines[4],
        "kiln++: note: please submit a bug report to https://github.com/christopherfretz/kiln/issues and include the crash reproducer"
    );
    assert_eq!(lines[5], "kiln++: note: set KILN_BACKTRACE=1 to include the compiler backtrace");
    assert_eq!(
        lines[6],
        "kiln++: note: preprocessed source and associated run script are located at:"
    );
    assert!(!err.contains("panicked at"), "{err}");
    // The directory is created, and holds exactly the two listed files.
    let written = files(&crash);
    assert_eq!(written, listed(&err));
    assert_eq!(written.len(), 2, "{err}");
    let ii = &written[0];
    let sh = &written[1];
    let stem = ii.file_name().unwrap().to_string_lossy().into_owned();
    assert!(stem.starts_with("a-") && stem.ends_with(".ii"), "{stem}");
    assert_eq!(sh.with_extension("ii"), *ii);
    // The .ii is the preprocessed input, with line markers.
    let pre = std::fs::read_to_string(ii).unwrap();
    assert!(pre.contains("int first = 1;"), "{pre}");
    assert!(pre.contains("# 1 \"a.cpp\""), "{pre}");
    // The .sh replays the command line on the .ii, quoting as needed.
    let script = std::fs::read_to_string(sh).unwrap();
    assert!(script.contains(&format!("cd '{}'", d.display())) || script.contains("cd /"));
    assert!(script.contains("'-DFOO=a b'"), "{script}");
    assert!(script.contains(&ii.display().to_string()), "{script}");
    assert!(!script.lines().last().unwrap().ends_with(" a.cpp"), "{script}");
    // Replaying it crashes the same way (and writes a second reproducer).
    let again = run(Path::new("sh"), &d, &d, &[&sh.display().to_string()], &[]);
    assert_eq!(again.status.code(), Some(1));
    assert!(stderr(&again).contains("crash requested by -fkiln-crash-for-testing=parse"));
    assert_eq!(files(&crash).len(), 4);
}

#[test]
fn tmpdir_is_the_default_directory() {
    let d = workdir("tmpdir");
    let tmp = d.join("tmp");
    std::fs::create_dir_all(&tmp).unwrap();
    let o =
        run(&kilnpp(), &d, &tmp, &["-fsyntax-only", "-fkiln-crash-for-testing=oven", "a.cpp"], &[]);
    let err = stderr(&o);
    assert_eq!(o.status.code(), Some(1), "{err}");
    assert!(err.contains("(phase: oven, at a.cpp:3:"), "{err}");
    assert_eq!(files(&tmp), listed(&err));
    assert_eq!(files(&tmp).len(), 2);
}

#[test]
fn no_crash_diagnostics_writes_nothing_and_exits_70() {
    let d = workdir("off");
    let tmp = d.join("tmp");
    std::fs::create_dir_all(&tmp).unwrap();
    let o = run(
        &kilnpp(),
        &d,
        &tmp,
        &["-fsyntax-only", "-fkiln-crash-for-testing=sema", "-fno-crash-diagnostics", "a.cpp"],
        &[],
    );
    let err = stderr(&o);
    assert_eq!(o.status.code(), Some(70), "{err}");
    assert!(err.contains("internal compiler error: "), "{err}");
    assert!(err.contains("(phase: sema)"), "{err}");
    assert!(!err.contains("diagnostic msg:"), "{err}");
    assert!(files(&tmp).is_empty());
}

#[test]
fn backtrace_only_on_request() {
    let d = workdir("backtrace");
    let args =
        ["-fsyntax-only", "-fkiln-crash-for-testing=parse", "-fno-crash-diagnostics", "a.cpp"];
    for var in ["KILN_BACKTRACE", "RUST_BACKTRACE"] {
        let o = run(&kilnpp(), &d, &d, &args, &[(var, "1")]);
        let err = stderr(&o);
        assert!(err.contains("kiln++: note: compiler backtrace:"), "{var}: {err}");
        assert!(!err.contains("set KILN_BACKTRACE=1"), "{var}: {err}");
    }
    let o = run(&kilnpp(), &d, &d, &args, &[("KILN_BACKTRACE", "0")]);
    assert!(!stderr(&o).contains("compiler backtrace:"));
}

#[test]
fn preprocessing_crash_writes_only_the_script() {
    let d = workdir("pp");
    let crash = d.join("crash");
    let dir_flag = format!("-fcrash-diagnostics-dir={}", crash.display());
    let o = run(
        &kilnpp(),
        &d,
        &d,
        &["-fsyntax-only", "-fkiln-crash-for-testing=preprocess", &dir_flag, "a.cpp"],
        &[],
    );
    let err = stderr(&o);
    assert_eq!(o.status.code(), Some(1), "{err}");
    assert!(
        err.contains(
            "note: diagnostic msg: preprocessing the input also crashed; only the run script was written"
        ),
        "{err}"
    );
    let written = files(&crash);
    assert_eq!(written, listed(&err));
    assert_eq!(written.len(), 1);
    assert!(written[0].extension().is_some_and(|e| e == "sh"));
}

#[test]
fn standard_input() {
    let d = workdir("stdin");
    let crash = d.join("crash");
    let dir_flag = format!("-fcrash-diagnostics-dir={}", crash.display());
    let mut c = Command::new(kilnpp());
    c.current_dir(&d)
        .args(["-fsyntax-only", "-fkiln-crash-for-testing=parse", &dir_flag, "-x", "c++", "-"])
        .env_remove("RUST_BACKTRACE")
        .env_remove("KILN_BACKTRACE")
        .stdin(std::fs::File::open(d.join("a.cpp")).unwrap());
    let o = c.output().unwrap();
    let err = stderr(&o);
    assert_eq!(o.status.code(), Some(1), "{err}");
    assert!(err.contains("note: while compiling '<stdin>' (phase: parse"), "{err}");
    let written = files(&crash);
    assert_eq!(written, listed(&err));
    assert_eq!(written.len(), 2, "{err}");
    let name = written[0].file_name().unwrap().to_string_lossy().into_owned();
    assert!(name.starts_with("stdin-"), "{name}");
    assert!(std::fs::read_to_string(&written[0]).unwrap().contains("int first = 1;"));
}
