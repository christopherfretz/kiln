//! A panic in the in-process back end is contained by the driver
//! (spec/driver.md "In-process back end", the contained-panic scope) and
//! the next compile in the same process is hod's again, byte for byte.
//!
//! hod contains its own panics (`CompileError::Panic`, reason
//! `internal-panic`), and its forced-panic switch is compiled into hod's
//! own tests only; so the panic here is forced in front of hod: a back end
//! that panics on its first call and is [`kiln_hod::Hod`] afterwards. The
//! hook is process-global, so the compiles run in a child: this test
//! binary re-executed (`child`, active only with `KILN_HOD_PANIC_RUNS`).
#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};

use kiln_driver::backend_hook::{Answer, InProcessBackend, register};

/// Panics on the first call, then compiles with hod.
#[derive(Debug, Default)]
struct PanicOnce {
    panicked: AtomicBool,
    hod: kiln_hod::Hod,
}

impl InProcessBackend for PanicOnce {
    fn compile(
        &self,
        module: &kiln_llvm::Module,
        llc_args: &[String],
        object: &mut Vec<u8>,
    ) -> Answer {
        if !self.panicked.swap(true, Ordering::SeqCst) {
            object.extend_from_slice(b"PARTIAL");
            panic!("forced panic in front of hod");
        }
        self.hod.compile(module, llc_args, object)
    }
}

/// Separates runs in `KILN_HOD_PANIC_RUNS`; arguments by `\u{1f}`.
const RUN_SEP: char = '\u{1e}';
const ARG_SEP: char = '\u{1f}';

/// The child: every run of `KILN_HOD_PANIC_RUNS` in this process, as the
/// wrapper's `main` runs one (crash hook, then the driver).
#[test]
fn child() {
    let Ok(runs) = std::env::var("KILN_HOD_PANIC_RUNS") else { return };
    kiln_driver::crash::install_hook();
    assert!(register(Box::new(PanicOnce::default())).is_ok());
    let mut codes = Vec::new();
    for run in runs.split(RUN_SEP) {
        let raw: Vec<String> = run.split(ARG_SEP).map(str::to_owned).collect();
        let code = match std::panic::catch_unwind(|| {
            kiln_driver::run_as("kiln++", &raw, kiln_driver::Io::std())
        }) {
            Ok(c) => c,
            Err(_) => kiln_driver::crash::report("kiln++", &raw),
        };
        codes.push(code);
    }
    std::process::exit(i32::from(codes.iter().any(|&c| c != 0)));
}

fn have_llvm() -> bool {
    let kiln = PathBuf::from(env!("CARGO_BIN_EXE_kiln"));
    let out = Command::new(kiln).arg("--version").output().expect("run the wrapper");
    let ok = String::from_utf8_lossy(&out.stdout)
        .lines()
        .any(|l| l.starts_with("LLVM backend: ") && !l.contains("none"));
    if !ok {
        eprintln!("skipping: the wrapper found no LLVM installation");
    }
    ok
}

#[test]
fn a_panic_is_contained_and_the_next_compile_is_hods() {
    if !have_llvm() {
        return;
    }
    let dir = std::env::temp_dir().join(format!("kiln-hod-panic-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/programs/virtual.cpp");
    let run = |obj: &str| {
        let d = dir.display();
        ["-fkiln-backend=hod", "-c", &src.display().to_string(), "-o", &format!("{d}/{obj}")]
            .join(&ARG_SEP.to_string())
    };
    let runs = [run("first.o"), run("second.o"), run("third.o")].join(&RUN_SEP.to_string());
    let log = dir.join("stats.jsonl");
    let out = Command::new(std::env::current_exe().unwrap())
        .args(["child", "--exact", "--nocapture", "--test-threads=1"])
        .env("KILN_HOD_PANIC_RUNS", runs)
        .env("KILN_STATS_LOG", &log)
        .env_remove("KILN_HOD_STRICT")
        .env_remove("KILN_BACKEND")
        .output()
        .unwrap();
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "{}\n{err}", out.status);
    // The panic: a warning, the fallback, no crash report, no panic
    // message from the hook.
    assert!(
        err.contains(
            "warning: hod failed (in process): internal-panic: forced panic in front of hod"
        ),
        "{err}"
    );
    assert!(!err.contains("PLEASE submit a bug report"), "{err}");
    assert!(!err.contains("panicked at"), "{err}");
    let stats = std::fs::read_to_string(&log).unwrap();
    let lines: Vec<&str> = stats.lines().collect();
    assert_eq!(lines.len(), 3, "{stats}");
    assert!(lines[0].contains("\"backend\":\"hod-failed\""), "{}", lines[0]);
    assert!(lines[0].contains("\"hod_reason\":\"internal-panic\""), "{}", lines[0]);
    for l in &lines[1..] {
        assert!(l.contains("\"backend\":\"hod\""), "{l}");
        assert!(l.contains("\"in_process\":true"), "{l}");
    }
    // The fallback's object is LLVM's, not the partial bytes; the two
    // compiles after the panic give the same bytes.
    let first = std::fs::read(dir.join("first.o")).unwrap();
    assert!(first.starts_with(b"\x7fELF"));
    let second = std::fs::read(dir.join("second.o")).unwrap();
    let third = std::fs::read(dir.join("third.o")).unwrap();
    assert!(second == third, "hod's objects after the panic differ");
    // ... and are a fresh process's.
    let fresh = dir.join("fresh.o");
    let fresh_status = Command::new(env!("CARGO_BIN_EXE_kiln"))
        .args(["--driver-mode=g++", "-fkiln-backend=hod", "-c"])
        .arg(&src)
        .arg("-o")
        .arg(&fresh)
        .env_remove("KILN_STATS_LOG")
        .status()
        .unwrap();
    assert!(fresh_status.success());
    assert!(std::fs::read(&fresh).unwrap() == second, "a fresh process's object differs");
    let _ = std::fs::remove_dir_all(&dir);
}
