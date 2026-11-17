//! The in-process back-end hook and its route (spec/driver.md "In-process
//! back end", WP-1559), with FAKE back ends.
//!
//! The hook is process-global, so each compile runs in a child process:
//! this test binary re-executed as a wrapper `kiln++` (the `wrapper` test
//! below, active only when `KILN_HOOK_TEST_RUNS` is set) that installs
//! the crash hook, registers [`Fake`] and runs the driver as the `kiln`
//! binary's `main` does (a panic becomes the crash report). The fake's
//! answer is `FAKE_BACKEND` (`take`, `decline`, `fail`, `panic`,
//! `panic-thread`); every call appends its `llc` arguments to `FAKE_LOG`.
//! Tests whose route runs `llc` skip without `llc-19`, `opt-19` and
//! `clang++-19`; the others use a stub LLVM installation.
#![forbid(unsafe_code)]

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use kiln_driver::backend_hook::{Answer, InProcessBackend, register};

/// Separates the arguments of one run in `KILN_HOOK_TEST_RUNS`.
const ARG_SEP: char = '\u{1f}';
/// Separates runs.
const RUN_SEP: char = '\u{1e}';

struct Fake;

impl InProcessBackend for Fake {
    fn compile(
        &self,
        module: &kiln_llvm::Module,
        llc_args: &[String],
        object: &mut Vec<u8>,
    ) -> Answer {
        if let Some(log) = std::env::var_os("FAKE_LOG") {
            use std::io::Write as _;
            let mut f = std::fs::OpenOptions::new().append(true).create(true).open(log).unwrap();
            writeln!(f, "{} {}", module.source_filename, llc_args.join(" ")).unwrap();
        }
        match std::env::var("FAKE_BACKEND").as_deref() {
            Ok("take") => {
                object.extend_from_slice(b"FAKEOBJ");
                Answer::Done
            }
            Ok("decline") => Answer::Declined("fake:declined".to_owned()),
            Ok("fail") => {
                object.extend_from_slice(b"PARTIAL");
                Answer::Failed("internal error: boom".to_owned())
            }
            Ok("panic") => panic!("the fake back end panicked"),
            Ok("panic-thread") => std::thread::scope(|s| {
                s.spawn(|| panic!("a worker of the fake back end panicked"));
                Answer::Done
            }),
            v => panic!("FAKE_BACKEND={v:?}"),
        }
    }
}

/// The wrapper `kiln++` (a no-op in a normal test run): runs each run of
/// `KILN_HOOK_TEST_RUNS` in this process, in order, stopping at the
/// first non-zero status, and exits with it.
#[test]
fn wrapper() {
    let Ok(runs) = std::env::var("KILN_HOOK_TEST_RUNS") else { return };
    kiln_driver::crash::install_hook();
    assert!(register(Box::new(Fake)).is_ok());
    let mut code = 0;
    for run in runs.split(RUN_SEP) {
        let raw: Vec<String> = run.split(ARG_SEP).map(str::to_owned).collect();
        code = match std::panic::catch_unwind(|| {
            kiln_driver::run_as("kiln++", &raw, kiln_driver::Io::std())
        }) {
            Ok(c) => c,
            Err(_) => kiln_driver::crash::report("kiln++", &raw),
        };
        if code != 0 {
            break;
        }
    }
    std::process::exit(code);
}

fn have(tool: &str) -> bool {
    Command::new(tool).arg("--version").stdout(Stdio::null()).stderr(Stdio::null()).status().is_ok()
}

fn tools() -> bool {
    let ok = have("llc-19") && have("opt-19") && have("clang++-19");
    if !ok {
        eprintln!("skipping: llc-19 / opt-19 / clang++-19 not found");
    }
    ok
}

const PLAIN: &str = "int add(int a, int b) { return a + b; }\n";

const INLINE: &str = "[[gnu::always_inline]] inline int twice(int x) { return 2 * x; }\n\
                      int f(int x) { return twice(x) + twice(x + 1); }\n";

/// A call site marked `always_inline` (a `[[clang::always_inline]]`
/// statement), the callee an ordinary function.
const CALL_SITE: &str = "int g(int x) { return x + 1; }\n\
                         int f(int x) { [[clang::always_inline]] return g(x); }\n";

fn scratch(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("kiln-hook-test-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    std::fs::write(d.join("plain.cpp"), PLAIN).unwrap();
    std::fs::write(d.join("inline.cpp"), INLINE).unwrap();
    std::fs::write(d.join("call_site.cpp"), CALL_SITE).unwrap();
    d
}

fn script(path: &Path, body: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, format!("#!/bin/sh\n{body}\n")).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

/// A stub LLVM installation at `<dir>/llvm` whose `llc` and `opt` append
/// their name to `<dir>/llvm.ran` (and write nothing).
fn stub_llvm(dir: &Path) -> String {
    let llvm = dir.join("llvm");
    for t in ["opt", "llc"] {
        script(
            &llvm.join("bin").join(t),
            &format!("cat > /dev/null\necho {t} >> '{}/llvm.ran'", dir.display()),
        );
    }
    script(&llvm.join("bin/clang++"), "exit 0");
    std::fs::create_dir_all(llvm.join("lib/clang/22")).unwrap();
    format!("--llvm-dir={}", llvm.display())
}

/// Runs the wrapper in `dir` with the fake answering `fake`, over
/// `runs` (each a command line), with `env`.
fn wrap(dir: &Path, fake: &str, env: &[(&str, &str)], runs: &[&[&str]]) -> Output {
    let runs: Vec<String> = runs.iter().map(|r| r.join(&ARG_SEP.to_string())).collect();
    let mut c = Command::new(std::env::current_exe().unwrap());
    c.args(["--exact", "wrapper", "--nocapture", "--test-threads=1"])
        .current_dir(dir)
        .env_remove("KILN_BACKEND")
        .env_remove("KILN_HOD")
        .env_remove("KILN_HOD_STRICT")
        .env_remove("KILN_STATS_LOG")
        .env_remove("RUST_BACKTRACE")
        .env_remove("KILN_BACKTRACE")
        .env("KILN_HOOK_TEST_RUNS", runs.join(&RUN_SEP.to_string()))
        .env("FAKE_BACKEND", fake)
        .env("FAKE_LOG", dir.join("fake.log"))
        .stdin(Stdio::null());
    for (k, v) in env {
        c.env(k, v);
    }
    c.output().unwrap()
}

fn text(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}

fn read(p: &Path) -> Vec<u8> {
    std::fs::read(p).unwrap_or_default()
}

/// Done: the fake's bytes are the `-o` file; no `opt`, no `llc`, no
/// print (`-ftime-trace` has no `PrintIR`, the statistics line no
/// `ir_bytes` and its own `in-process-backend` time); `-v` shows the
/// in-process step with `llc`'s arguments; the external executable is
/// never looked for (none exists).
#[test]
fn takes_the_job() {
    let d = scratch("take");
    let llvm = stub_llvm(&d);
    let log = d.join("s.jsonl");
    let env = [("KILN_STATS_LOG", log.to_str().unwrap())];
    // Granularity 0: the fake answers in microseconds, under the default
    // 500 us an event needs to be kept.
    let args = [
        "-c",
        "-v",
        "-ftime-trace",
        "-ftime-trace-granularity=0",
        "-fkiln-backend=hod",
        &llvm,
        "plain.cpp",
        "-o",
        "p.o",
    ];
    let o = wrap(&d, "take", &env, &[&args]);
    let err = text(&o.stderr);
    assert!(o.status.success(), "{err}");
    assert_eq!(read(&d.join("p.o")), b"FAKEOBJ");
    assert!(!d.join("llvm.ran").exists(), "{err}");
    let calls = text(&read(&d.join("fake.log")));
    assert_eq!(calls.lines().count(), 1, "{calls}");
    assert!(
        calls.contains("-O0") && calls.contains("-filetype=obj") && calls.ends_with("-o p.o -\n")
    );
    assert!(err.contains(" (in-process) hod -O0 "), "{err}");
    assert!(err.contains(" -filetype=obj ") && err.contains(" -o p.o -\n"), "{err}");
    let trace = text(&read(&d.join("p.json")));
    assert!(trace.contains("\"name\":\"CodeGenPasses\""), "{trace}");
    assert!(trace.contains("\"detail\":\"in-process\""), "{trace}");
    assert!(!trace.contains("PrintIR"), "{trace}");
    assert!(trace.contains("\"name\":\"EmitIR\""), "{trace}");
    let line = text(&read(&log));
    assert!(line.contains("\"ir_bytes\":null"), "{line}");
    assert!(line.contains("\"print_ir_ms\":0.000"), "{line}");
    assert!(line.contains("\"children_ms\":{\"in-process-backend\":"), "{line}");
    assert!(
        line.ends_with(
            ",\"backend\":\"hod\",\"hod_reason\":null,\"hod_skip\":null,\"in_process\":true}\n"
        ),
        "{line}"
    );
    // `-###`: the in-process step, then the fallback mark and `llc`.
    let o = wrap(&d, "take", &[], &[&["-###", "-c", "-fkiln-backend=hod", &llvm, "plain.cpp"]]);
    let err = text(&o.stderr);
    let at = |s: &str| err.find(s).unwrap_or_else(|| panic!("{s}: {err}"));
    let step = at(" (in-process) hod \"-O0\"");
    assert!(at(" (in-process) frontend") < step, "{err}");
    assert!(step < at(" (fallback if hod declines or fails") && step < at("/llc\""), "{err}");
    // The LLVM back end, a hook registered: untouched.
    let o = wrap(&d, "take", &[], &[&["-c", "-fkiln-backend=llvm", &llvm, "plain.cpp"]]);
    assert!(o.status.success(), "{}", text(&o.stderr));
    assert_eq!(text(&read(&d.join("fake.log"))).lines().count(), 1);
    assert_eq!(text(&read(&d.join("llvm.ran"))), "llc\n");
    let _ = std::fs::remove_dir_all(&d);
}

/// Jobs outside the selection rule never reach the hook: an `-O` level
/// above 0, a sanitizer, `-S`, an `always_inline` function and an
/// `always_inline` call site (`opt` would run).
#[test]
fn outside_the_rule_never_reaches_the_hook() {
    if !tools() {
        return;
    }
    let d = scratch("outside");
    for extra in [
        &["-O2", "plain.cpp"][..],
        &["-fsanitize=address", "plain.cpp"],
        &["-S", "plain.cpp"],
        &["inline.cpp"],
        &["call_site.cpp"],
    ] {
        let args = [&["-c", "-fkiln-backend=hod", "-o", "x.o"][..], extra].concat();
        let o = wrap(&d, "take", &[], &[&args]);
        assert!(o.status.success(), "{extra:?}: {}", text(&o.stderr));
        assert!(!d.join("fake.log").exists(), "{extra:?}");
    }
    let log = d.join("s.jsonl");
    let env = [("KILN_STATS_LOG", log.to_str().unwrap())];
    let o = wrap(&d, "take", &env, &[&["-c", "-fkiln-backend=hod", "call_site.cpp", "-o", "x.o"]]);
    assert!(o.status.success(), "{}", text(&o.stderr));
    let line = text(&read(&log));
    assert!(line.contains("\"triggers\":[\"alwaysinline\"]"), "{line}");
    assert!(
        line.ends_with(
            ",\"backend\":\"llvm\",\"hod_reason\":null,\"hod_skip\":\"alwaysinline\"}\n"
        )
    );
    let _ = std::fs::remove_dir_all(&d);
}

/// Declined: the LLVM route, byte-identical to `-fkiln-backend=llvm`, no
/// diagnostic; `-v` names the reason.
#[test]
fn declined_takes_the_llvm_route() {
    if !tools() {
        return;
    }
    let d = scratch("decline");
    let llvm = ["-c", "-fkiln-backend=llvm", "plain.cpp", "-o", "llvm.o"];
    let hod = ["-c", "-fkiln-backend=hod", "plain.cpp", "-o", "hod.o"];
    let o = wrap(&d, "decline", &[], &[&llvm, &hod]);
    assert!(o.status.success(), "{}", text(&o.stderr));
    assert_eq!(text(&o.stderr), "");
    assert!(!read(&d.join("llvm.o")).is_empty());
    assert_eq!(read(&d.join("llvm.o")), read(&d.join("hod.o")));
    let o = wrap(&d, "decline", &[], &[&["-v", "-c", "-fkiln-backend=hod", "plain.cpp"]]);
    let err = text(&o.stderr);
    let at = |s: &str| err.find(s).unwrap_or_else(|| panic!("{s}: {err}"));
    let reason = at(" (hod declined 'fake:declined'; falling back to llc)\n");
    assert!(at(" (in-process) hod ") < reason && reason < at("llc"), "{err}");
    let _ = std::fs::remove_dir_all(&d);
}

/// Failed: one warning, then the LLVM route (the object equal to
/// `-fkiln-backend=llvm`'s); under `KILN_HOD_STRICT=1` or
/// `-Werror=hod-fallback` an error, and no `-o` file.
#[test]
fn failure_warns_and_falls_back() {
    if !tools() {
        return;
    }
    let d = scratch("fail");
    let llvm = ["-c", "-fkiln-backend=llvm", "plain.cpp", "-o", "llvm.o"];
    let hod = ["-c", "-fkiln-backend=hod", "plain.cpp", "-o", "hod.o"];
    let o = wrap(&d, "fail", &[], &[&llvm, &hod]);
    assert!(o.status.success(), "{}", text(&o.stderr));
    assert_eq!(
        text(&o.stderr),
        "kiln++: warning: hod failed (in process): internal error: boom; falling back to llc \
         [-Whod-fallback]\n"
    );
    assert_eq!(read(&d.join("llvm.o")), read(&d.join("hod.o")));
    std::fs::write(d.join("stale.o"), "STALE").unwrap();
    let strict = ["-c", "-fkiln-backend=hod", "plain.cpp", "-o", "stale.o"];
    let o = wrap(&d, "fail", &[("KILN_HOD_STRICT", "1")], &[&strict]);
    assert_eq!(o.status.code(), Some(1), "{}", text(&o.stderr));
    assert!(text(&o.stderr).contains("(KILN_HOD_STRICT=1: no fallback)"));
    assert!(!d.join("stale.o").exists());
    let werror = [&strict[..], &["-Werror=hod-fallback"]].concat();
    let o = wrap(&d, "fail", &[], &[&werror]);
    assert_eq!(o.status.code(), Some(1), "{}", text(&o.stderr));
    assert!(!d.join("stale.o").exists());
    let _ = std::fs::remove_dir_all(&d);
}

/// A panic in the back end, on its own thread or on a thread it started:
/// contained, the warning with `internal-panic`, the LLVM route's object;
/// then a deliberate crash later in the same process is reported as its
/// own (its message in the banner, its reproducer written), not as the
/// back end's.
#[test]
fn panics_are_contained() {
    if !tools() {
        return;
    }
    for fake in ["panic", "panic-thread"] {
        let d = scratch(fake);
        let log = d.join("s.jsonl");
        let crash_dir = d.join("crash");
        let crash_flag = format!("-fcrash-diagnostics-dir={}", crash_dir.display());
        let llvm = ["-c", "-fkiln-backend=llvm", "plain.cpp", "-o", "llvm.o"];
        let hod = ["-c", "-fkiln-backend=hod", "plain.cpp", "-o", "hod.o"];
        let crash = ["-c", "-fkiln-crash-for-testing=sema", &crash_flag, "plain.cpp", "-o", "x.o"];
        let env = [("KILN_STATS_LOG", log.to_str().unwrap())];
        let o = wrap(&d, fake, &env, &[&llvm, &hod, &crash]);
        let err = text(&o.stderr);
        let message = if fake == "panic" {
            "the fake back end panicked"
        } else {
            "a worker of the fake back end panicked"
        };
        let warning = format!(
            "kiln++: warning: hod failed (in process): internal-panic: {message}; falling back \
             to llc [-Whod-fallback]\n"
        );
        assert!(err.starts_with(&warning), "{fake}: {err}");
        assert_eq!(read(&d.join("llvm.o")), read(&d.join("hod.o")), "{fake}");
        let stats = text(&read(&log));
        assert!(
            stats.contains(
                "\"backend\":\"hod-failed\",\"hod_reason\":\"internal-panic\",\"hod_skip\":null,\
                 \"in_process\":true}"
            ),
            "{stats}"
        );
        // The later crash: its own report.
        assert_eq!(o.status.code(), Some(1), "{fake}: {err}");
        let banner = &err[warning.len()..];
        assert!(
            banner.starts_with(
                "kiln++: error: internal compiler error: crash requested by \
                 -fkiln-crash-for-testing=sema\n"
            ),
            "{fake}: {banner}"
        );
        assert!(!banner.contains("fake back end"), "{fake}: {banner}");
        let written = std::fs::read_dir(&crash_dir).map_or(0, Iterator::count);
        assert_eq!(written, 2, "{fake}: the .ii and the .sh: {banner}");
        let _ = std::fs::remove_dir_all(&d);
    }
}
