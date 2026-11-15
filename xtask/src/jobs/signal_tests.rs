//! The signal path end to end (`spec/harness.md` "Jobserver", Signals;
//! WP-1436): each test re-executes this test binary as a probe that sets up
//! the signal thread, a jobserver fifo of [`POOL`] tokens and stub children
//! (`sh` loops, no kiln build) in one phase of a real run, delivers SIGTERM
//! or SIGINT, and asserts that the probe is gone within the deadline, died
//! of that signal, left no child running and returned every token exactly
//! once. Synchronization is by polling a condition with a generous
//! deadline, never by a sleep.

use super::refill;
use crate::util::process;
use std::fs::{File, OpenOptions};
use std::io::{Read as _, Write as _};
use std::os::unix::process::ExitStatusExt as _;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};

const PROBE: &str = "KILN_XTASK_SIGNAL_PROBE";
const PROBE_DIR: &str = "KILN_XTASK_SIGNAL_PROBE_DIR";
const LIMITS: &str = "KILN_XTASK_TEST_SIGNAL_MS";
const SIGINT: i32 = 2;
const SIGTERM: i32 = 15;

/// The probe's jobserver pool.
const POOL: usize = 4;

/// How long any one wait of these tests may take (a loaded machine).
const DEADLINE: Duration = Duration::from_secs(60);

/// The re-executed side: does nothing in a normal test run (no [`PROBE`]).
#[test]
fn probe() {
    let (Ok(phase), Some(dir)) = (std::env::var(PROBE), std::env::var_os(PROBE_DIR)) else {
        return;
    };
    let dir = PathBuf::from(dir);
    assert!(super::signals_caught());
    match phase.as_str() {
        // `util::build_kiln`: a cargo step, stderr inherited.
        "build" => drop(process::run_step(&mut stub(&dir, "step", "", ""), false)),
        // A `cargo xtask ci` step: stderr piped through, line by line.
        "ci-step" => drop(process::run_step(&mut stub(&dir, "step", "", "echo line >&2;"), true)),
        // A cargo step that ignores SIGTERM (only SIGKILL stops it).
        "ignores-term" => {
            drop(process::run_step(&mut stub(&dir, "step", "trap '' TERM;", ""), false));
        }
        // `xtask test` workers, each with a compiler running (and a token).
        "tests" => std::thread::scope(|s| {
            for i in 0..3 {
                let dir = &dir;
                s.spawn(move || {
                    let mut c = stub(dir, &format!("test{i}"), "", "");
                    drop(process::run(&mut c, Duration::from_secs(600)));
                });
            }
        }),
        // Workers starting short children back to back: the signal lands
        // between a child's exit and the next spawn, or in a spawn.
        "between" => std::thread::scope(|s| {
            for _ in 0..2 {
                let dir = &dir;
                s.spawn(move || {
                    let script = format!("echo $$ >> {}", dir.join("spawned").display());
                    let mut c = Command::new("sh");
                    c.args(["-c", &script, &marker(dir)]);
                    while process::run(&mut c, Duration::from_secs(600)).is_ok() {}
                });
            }
        }),
        other => panic!("unknown probe phase {other}"),
    }
    // What `main` does once its command returns.
    let _ = super::finish(std::process::ExitCode::SUCCESS);
}

/// The `$0` of every stub shell, so a pid is only taken for ours.
fn marker(dir: &Path) -> String {
    format!("kiln-signal-probe-{}", dir.file_name().unwrap().to_string_lossy())
}

/// `sh -c '<pre> echo $$ > <dir>/<name>.pid; while :; do <body> sleep 0.1; done'`.
fn stub(dir: &Path, name: &str, pre: &str, body: &str) -> Command {
    let pid_file = dir.join(format!("{name}.pid"));
    let script =
        format!("{pre} echo $$ > {}; while :; do {body} sleep 0.1; done", pid_file.display());
    let mut c = Command::new("sh");
    c.args(["-c", &script, &marker(dir)]);
    c
}

/// How one probe run is set up and what is done to it.
#[derive(Default)]
struct Case {
    phase: &'static str,
    signal: i32,
    /// stderr is a pipe whose reader is gone when the signal arrives.
    dead_reader: bool,
    /// `KILN_XTASK_TEST_SIGNAL_MS` (reap, term, bound).
    limits: Option<(u64, u64, u64)>,
    /// Deliver the signal a second time, once the cleanup is under way.
    twice: bool,
}

struct Outcome {
    status: ExitStatus,
    stderr: String,
    /// From the (last) signal to the probe's end.
    after_signal: Duration,
}

/// Removes the probe's directory.
struct Cleanup(PathBuf);
impl Drop for Cleanup {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn wait_for(what: &str, child: &mut Child, mut ready: impl FnMut() -> bool) {
    let start = Instant::now();
    while !ready() {
        if let Ok(Some(s)) = child.try_wait() {
            panic!("the probe ended ({s}) before {what}");
        }
        if start.elapsed() > DEADLINE {
            let _ = child.kill();
            let _ = child.wait();
            panic!("timed out waiting for {what}");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn send(pid: u32, sig: i32) {
    let ok = Command::new("kill").args([format!("-{sig}"), pid.to_string()]).status();
    assert!(ok.is_ok_and(|s| s.success()), "kill -{sig} {pid}");
}

/// Our stub with that pid is still running (not gone, not a zombie).
fn alive(pid: u32, marker: &str) -> bool {
    let proc = PathBuf::from(format!("/proc/{pid}"));
    let state = std::fs::read_to_string(proc.join("stat")).unwrap_or_default();
    let zombie = state.rsplit_once(')').is_some_and(|(_, rest)| rest.trim_start().starts_with('Z'));
    let cmdline = std::fs::read(proc.join("cmdline")).unwrap_or_default();
    !zombie && String::from_utf8_lossy(&cmdline).contains(marker)
}

fn pids(dir: &Path) -> Vec<u32> {
    let mut v = Vec::new();
    for e in std::fs::read_dir(dir).unwrap().flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        if e.path().extension().is_some_and(|x| x == "pid") || name == "spawned" {
            let text = std::fs::read_to_string(e.path()).unwrap_or_default();
            v.extend(text.lines().filter_map(|l| l.trim().parse::<u32>().ok()));
        }
    }
    v
}

/// The probe has a thread of that name (the cleanup has started). Linux
/// keeps the first 15 bytes of a thread name.
fn has_thread(pid: u32, name: &str) -> bool {
    let name = &name[..name.len().min(15)];
    let Ok(tasks) = std::fs::read_dir(format!("/proc/{pid}/task")) else { return false };
    tasks
        .flatten()
        .any(|t| std::fs::read_to_string(t.path().join("comm")).is_ok_and(|c| c.trim_end() == name))
}

fn run(case: &Case) -> Outcome {
    let tag = format!(
        "{}-{}-{}{}{}",
        std::process::id(),
        case.phase,
        case.signal,
        if case.dead_reader { "-dead" } else { "" },
        if case.twice { "-twice" } else { "" },
    );
    let dir = std::env::temp_dir().join(format!("kiln-xtask-signal-{tag}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let _rm = Cleanup(dir.clone());
    let fifo = dir.join("fifo");
    assert!(Command::new("mkfifo").arg(&fifo).status().unwrap().success(), "mkfifo");
    let mut holder: File = OpenOptions::new().read(true).write(true).open(&fifo).unwrap();
    holder.write_all(&[b'+'; POOL]).unwrap();

    let mut cmd = Command::new(std::env::current_exe().unwrap());
    cmd.args(["--exact", "jobs::signal_tests::probe", "--nocapture", "--test-threads=1", "-q"])
        .env(PROBE, case.phase)
        .env(PROBE_DIR, &dir)
        .env("MAKEFLAGS", format!("--jobserver-auth=fifo:{}", fifo.display()))
        .env_remove(LIMITS)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    if let Some((reap, term, bound)) = case.limits {
        cmd.env(LIMITS, format!("{reap},{term},{bound}"));
    }
    let mut child = cmd.spawn().unwrap();
    let pid = child.id();
    let mut pipe = child.stderr.take();
    let marker = marker(&dir);

    let phase_ready = || match case.phase {
        "tests" => (0..3).all(|i| dir.join(format!("test{i}.pid")).exists()),
        "between" => {
            std::fs::read_to_string(dir.join("spawned")).is_ok_and(|t| t.lines().count() >= 20)
        }
        _ => dir.join("step.pid").exists(),
    };
    wait_for(&format!("phase {}", case.phase), &mut child, phase_ready);
    let reader = if case.dead_reader {
        drop(pipe.take());
        None
    } else {
        let mut p = pipe.take().unwrap();
        Some(std::thread::spawn(move || {
            let mut s = String::new();
            let _ = p.read_to_string(&mut s);
            s
        }))
    };
    send(pid, case.signal);
    let mut signalled = Instant::now();
    if case.twice {
        // The second signal must come after the duplicate window, counted
        // from when the signal thread acted on the first.
        wait_for("the cleanup thread", &mut child, || has_thread(pid, super::CLEANUP_THREAD));
        let seen = Instant::now();
        wait_for("the duplicate window", &mut child, || {
            seen.elapsed() > super::DUPLICATE_WINDOW + Duration::from_millis(100)
        });
        send(pid, case.signal);
        signalled = Instant::now();
    }
    let status = loop {
        if let Some(s) = child.try_wait().unwrap() {
            break s;
        }
        if signalled.elapsed() > DEADLINE {
            let _ = child.kill();
            let _ = child.wait();
            panic!("the probe ({}) outlived its signal by {DEADLINE:?}", case.phase);
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    let after_signal = signalled.elapsed();
    let stderr = reader.map(|r| r.join().unwrap()).unwrap_or_default();

    // No stub survives (a killed one may stay a zombie until reaped).
    let ours = pids(&dir);
    assert!(!ours.is_empty(), "no stub pid recorded");
    let start = Instant::now();
    while let Some(p) = ours.iter().find(|&&p| alive(p, &marker)) {
        assert!(start.elapsed() < DEADLINE, "stub {p} survived the probe ({})", case.phase);
        std::thread::sleep(Duration::from_millis(10));
    }
    // Every token is back, exactly once.
    assert_eq!(refill(&fifo, None).unwrap().idle, POOL, "pool after {} ({stderr})", case.phase);
    drop(holder);
    Outcome { status, stderr, after_signal }
}

fn killed_by(o: &Outcome, sig: i32) {
    assert_eq!(o.status.signal(), Some(sig), "status {} ({})", o.status, o.stderr);
}

#[test]
fn sigterm_while_building() {
    let o = run(&Case { phase: "build", signal: SIGTERM, ..Case::default() });
    killed_by(&o, SIGTERM);
    assert!(o.stderr.contains("xtask: signal 15: killed 1 child process(es)"), "{}", o.stderr);
}

#[test]
fn sigint_while_building() {
    let o = run(&Case { phase: "build", signal: SIGINT, ..Case::default() });
    killed_by(&o, SIGINT);
}

/// The WP-1436 hang: the message after the kill hit EPIPE and panicked, the
/// signal thread died, and the cargo step's thread waited for it forever.
#[test]
fn sigterm_while_building_with_stderr_reader_gone() {
    let o = run(&Case { phase: "build", signal: SIGTERM, dead_reader: true, ..Case::default() });
    killed_by(&o, SIGTERM);
}

#[test]
fn sigterm_during_ci_step() {
    let o = run(&Case { phase: "ci-step", signal: SIGTERM, ..Case::default() });
    killed_by(&o, SIGTERM);
}

#[test]
fn sigint_during_ci_step_with_stderr_reader_gone() {
    let o = run(&Case { phase: "ci-step", signal: SIGINT, dead_reader: true, ..Case::default() });
    killed_by(&o, SIGINT);
}

#[test]
fn sigterm_while_tests_run() {
    let o = run(&Case { phase: "tests", signal: SIGTERM, ..Case::default() });
    killed_by(&o, SIGTERM);
    assert!(o.stderr.contains("killed 3 child process(es)"), "{}", o.stderr);
}

#[test]
fn sigint_while_tests_run_with_stderr_reader_gone() {
    let o = run(&Case { phase: "tests", signal: SIGINT, dead_reader: true, ..Case::default() });
    killed_by(&o, SIGINT);
}

#[test]
fn sigterm_between_spawns() {
    let o = run(&Case { phase: "between", signal: SIGTERM, ..Case::default() });
    killed_by(&o, SIGTERM);
}

/// A cargo step ignoring SIGTERM with a 60 s escalation: the second signal
/// ends the process at once, after `SIGKILL` to the step's group.
#[test]
fn second_signal_exits_at_once() {
    let o = run(&Case {
        phase: "ignores-term",
        signal: SIGTERM,
        limits: Some((60_000, 60_000, 60_000)),
        twice: true,
        ..Case::default()
    });
    killed_by(&o, SIGTERM);
    assert!(o.after_signal < Duration::from_secs(30), "{:?}", o.after_signal);
    assert!(o.stderr.contains("exiting without waiting for cleanup"), "{}", o.stderr);
}

/// The same stuck cleanup and no second signal: the bound ends it.
#[test]
fn stuck_cleanup_is_bounded() {
    let o = run(&Case {
        phase: "ignores-term",
        signal: SIGTERM,
        limits: Some((60_000, 60_000, 1_000)),
        ..Case::default()
    });
    killed_by(&o, SIGTERM);
    assert!(o.after_signal < Duration::from_secs(30), "{:?}", o.after_signal);
    assert!(o.stderr.contains("exiting anyway"), "{}", o.stderr);
}
