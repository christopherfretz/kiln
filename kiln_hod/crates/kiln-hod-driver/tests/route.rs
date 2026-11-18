//! The wrapper `kiln++` with hod in process, end to end (spec/driver.md
//! "In-process back end"): programs compiled through the in-process route
//! run as the LLVM route's do, a decline falls back and still links,
//! `KILN_HOD_STRICT=1` leaves a decline a silent fallback, the statistics
//! line names the route, and without `-fkiln-backend=hod` the wrapper is
//! the stock compiler.
//!
//! The tests need the wrapper's LLVM (its `llc` is the fallback) and a
//! linker: without an LLVM the wrapper's `--version` names none and they
//! skip, as `kiln_driver`'s tests skip without their tools. The comparison
//! with the stock binary needs `KILN_HOD_STOCK` (the stock `kiln++`; `cargo
//! xtask kiln-hod test` sets it) and skips without it.
#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// The wrapper, as `kiln++` (`kiln_driver`'s build script makes the name).
fn wrapper() -> PathBuf {
    let kiln = PathBuf::from(env!("CARGO_BIN_EXE_kiln"));
    let kilnpp = kiln.with_file_name("kiln++");
    if kilnpp.exists() { kilnpp } else { kiln }
}

/// Whether the wrapper found an LLVM (the fallback and the linker driver).
fn tools() -> bool {
    let out = Command::new(wrapper()).arg("--version").output().expect("run the wrapper");
    let text = String::from_utf8_lossy(&out.stdout);
    let ok = text.lines().any(|l| l.starts_with("LLVM backend: ") && !l.contains("none"));
    if !ok {
        eprintln!("skipping: the wrapper found no LLVM installation");
    }
    ok
}

fn program(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/programs").join(name)
}

fn scratch(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("kiln-hod-route-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Runs `cmd`, failing the test with its output if it does not exit 0.
fn ok(cmd: &mut Command) -> Output {
    let out = cmd.output().expect("run");
    assert!(
        out.status.success(),
        "{cmd:?} failed ({}):\n{}{}",
        out.status,
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    out
}

/// The wrapper compiling `src` to `obj` with `args`, its statistics line
/// in `dir/<obj>.jsonl`; returns the line.
fn compile(dir: &Path, src: &Path, obj: &str, args: &[&str], env: &[(&str, &str)]) -> String {
    let log = dir.join(format!("{obj}.jsonl"));
    let _ = std::fs::remove_file(&log);
    let mut cmd = Command::new(wrapper());
    cmd.current_dir(dir).args(args).arg("-c").arg(src).args(["-o", obj]);
    cmd.env("KILN_STATS_LOG", &log).env_remove("KILN_BACKEND").env_remove("KILN_HOD_STRICT");
    for (k, v) in env {
        cmd.env(k, v);
    }
    ok(&mut cmd);
    let text = std::fs::read_to_string(&log).unwrap();
    assert_eq!(text.lines().count(), 1, "{text}");
    text.trim_end().to_owned()
}

/// Links `obj` into `exe` with the wrapper and runs it: standard output
/// and exit status.
fn link_and_run(dir: &Path, obj: &str, exe: &str) -> (String, Option<i32>) {
    ok(Command::new(wrapper()).current_dir(dir).args([obj, "-pthread", "-o", exe]));
    let out = Command::new(dir.join(exe)).output().expect("run the program");
    (String::from_utf8_lossy(&out.stdout).into_owned(), out.status.code())
}

/// The programs, each with its expected output and status.
const PROGRAMS: &[(&str, &str, i32)] = &[
    ("hello.cpp", "hello, 42\n", 0),
    ("exceptions.cpp", "~guard\n~guard\n~guard\ncaught bottom\nrethrown bottom\n", 3),
    ("virtual.cpp", "square 9\nshape 10\n~Shape\n~Shape\n", 0),
    ("thread_local.cpp", "11 21 12 made=2\n", 0),
    (
        "init_order.cpp",
        "init first 1\ninit second 2\ninit local 3\nmain 3 3\nfini local\nfini second\nfini first\n",
        0,
    ),
];

/// Each program through the in-process route: hod takes it (strict, so a
/// failure would be an error), the statistics line says so, and the
/// program prints and returns what the LLVM route's does.
#[test]
fn programs_run_in_process() {
    if !tools() {
        return;
    }
    let dir = scratch("programs");
    for &(name, stdout, status) in PROGRAMS {
        let src = program(name);
        let line =
            compile(&dir, &src, "hod.o", &["-fkiln-backend=hod"], &[("KILN_HOD_STRICT", "1")]);
        assert!(line.contains("\"backend\":\"hod\""), "{name}: {line}");
        assert!(line.contains("\"in_process\":true"), "{name}: {line}");
        assert!(line.contains("\"children_ms\":{\"in-process-backend\":"), "{name}: {line}");
        let llvm = compile(&dir, &src, "llvm.o", &[], &[]);
        assert!(llvm.contains("\"backend\":\"llvm\""), "{name}: {llvm}");
        let hod = link_and_run(&dir, "hod.o", "hod");
        let reference = link_and_run(&dir, "llvm.o", "llvm");
        assert_eq!(hod, reference, "{name}: in process against LLVM");
        assert_eq!(hod, (stdout.to_owned(), Some(status)), "{name}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// A module hod declines (split DWARF 4: `debug:split-dwarf-4`, on every
/// target hod has) takes the LLVM route: the reason is in the statistics
/// line and `-v`, there is no diagnostic, and the object links and runs.
/// `KILN_HOD_STRICT=1` concerns failures only: a decline is the same
/// silent fallback under it.
#[test]
fn a_decline_falls_back() {
    if !tools() {
        return;
    }
    let dir = scratch("decline");
    let src = program("virtual.cpp");
    let args = ["-fkiln-backend=hod", "-gsplit-dwarf", "-gdwarf-4"];
    for strict in [None, Some(("KILN_HOD_STRICT", "1"))] {
        let env: Vec<(&str, &str)> = strict.into_iter().collect();
        let line = compile(&dir, &src, "d.o", &args, &env);
        assert!(line.contains("\"backend\":\"hod-declined\""), "{line}");
        assert!(line.contains("\"hod_reason\":\"debug:split-dwarf-4\""), "{line}");
        assert!(line.contains("\"in_process\":true"), "{line}");
        let (out, status) = link_and_run(&dir, "d.o", "d");
        assert_eq!((out.as_str(), status), ("square 9\nshape 10\n~Shape\n~Shape\n", Some(0)));
    }
    let out = ok(Command::new(wrapper())
        .current_dir(&dir)
        .args(args)
        .args(["-v", "-c"])
        .arg(&src)
        .args(["-o", "v.o"])
        .env_remove("KILN_HOD_STRICT"));
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains(" (in-process) hod "), "{err}");
    assert!(err.contains(" (hod declined 'debug:split-dwarf-4'; falling back to llc)"), "{err}");
    assert!(!err.contains("warning"), "{err}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// `--version`: the stock binary's lines, then one naming the built-in
/// back end and its revision.
#[test]
fn version_names_the_back_end() {
    let out = ok(Command::new(wrapper()).arg("--version"));
    let text = String::from_utf8_lossy(&out.stdout);
    let line =
        format!("In-process back end: hod {} ({})", kiln_hod::HOD_TAG, &kiln_hod::HOD_REV[..12]);
    assert_eq!(text.lines().filter(|l| l.starts_with("In-process back end: ")).count(), 1);
    assert!(text.lines().any(|l| l == line), "{text}");
    if let Some(stock) = stock() {
        let out = ok(Command::new(stock).arg("--version"));
        let stock = String::from_utf8_lossy(&out.stdout);
        let ours: Vec<&str> =
            text.lines().filter(|l| *l != line && !l.starts_with("InstalledDir:")).collect();
        let theirs: Vec<&str> = stock.lines().filter(|l| !l.starts_with("InstalledDir:")).collect();
        assert_eq!(ours, theirs);
    }
}

/// The stock `kiln++` (`KILN_HOD_STOCK`), if named.
fn stock() -> Option<PathBuf> {
    let p = std::env::var_os("KILN_HOD_STOCK").filter(|p| !p.is_empty()).map(PathBuf::from);
    if p.is_none() {
        eprintln!("skipping the stock comparison: KILN_HOD_STOCK is not set");
    }
    p
}

/// Without `-fkiln-backend=hod` the wrapper is the stock compiler: the
/// same object bytes as the stock binary's, for three files.
#[test]
fn without_the_flag_it_is_the_stock_compiler() {
    let Some(stock) = stock() else { return };
    if !tools() {
        return;
    }
    let dir = scratch("stock");
    for name in ["hello.cpp", "exceptions.cpp", "init_order.cpp"] {
        let src = program(name);
        for (exe, obj) in [(stock.clone(), "stock.o"), (wrapper(), "wrapper.o")] {
            ok(Command::new(exe)
                .current_dir(&dir)
                .args(["-c", "-g"])
                .arg(&src)
                .args(["-o", obj])
                .env_remove("KILN_BACKEND"));
        }
        let a = std::fs::read(dir.join("stock.o")).unwrap();
        let b = std::fs::read(dir.join("wrapper.o")).unwrap();
        assert!(a == b, "{name}: the wrapper's object differs from the stock binary's");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// `HOD_COVERAGE_LOG`: the in-process route appends the record the
/// external executable appends for the same job (outcome, reason, input
/// kind `text` and size, output, target), all but hod's time, for a job
/// hod takes and one it declines. Needs `KILN_HOD_EXTERNAL` (hod's
/// executable) for the external half.
#[test]
fn the_coverage_log_is_the_executables() {
    let Some(hod) = std::env::var_os("KILN_HOD_EXTERNAL").filter(|p| !p.is_empty()) else {
        eprintln!("skipping: KILN_HOD_EXTERNAL is not set");
        return;
    };
    if !tools() {
        return;
    }
    let dir = scratch("coverage");
    let src = program("virtual.cpp");
    let log = |name: &str, external: bool| {
        let log = dir.join(name);
        for extra in [&[][..], &["-gsplit-dwarf", "-gdwarf-4"][..]] {
            let mut cmd = Command::new(wrapper());
            cmd.current_dir(&dir).args(["-fkiln-backend=hod", "-c"]).args(extra).arg(&src);
            cmd.args(["-o", "c.o"]).env("HOD_COVERAGE_LOG", &log).env_remove("KILN_BACKEND");
            if external {
                cmd.env("KILN_HOD_IN_PROCESS", "0").env("KILN_HOD", &hod);
            }
            ok(&mut cmd);
        }
        // Field 7 is hod's time.
        std::fs::read_to_string(&log)
            .unwrap()
            .lines()
            .map(|l| {
                let mut f: Vec<String> = l.split('\t').map(str::to_owned).collect();
                f[6].clear();
                f
            })
            .collect::<Vec<_>>()
    };
    let external = log("external.tsv", true);
    let in_process = log("in-process.tsv", false);
    assert_eq!(external.len(), 2, "{external:?}");
    assert_eq!(in_process, external);
    let _ = std::fs::remove_dir_all(&dir);
}
