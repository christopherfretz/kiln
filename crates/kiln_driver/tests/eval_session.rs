//! WP-1995: evaluation sessions opened from a driver command line
//! (spec/eval-session.md §2): include paths, predefines and the
//! standard library apply as for `-fsyntax-only`, the host-call builtin
//! is visible, and a translation unit with errors is refused.
#![forbid(unsafe_code)]

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use kiln_driver::session::{SessionRefused, with_eval_session};
use kiln_driver::{Io, SharedWriter};
use kiln_parse::{CancelToken, EvalSession, HostCalls, HostError, HostOut, RunBudget, RunValue};

/// Standard error, kept for the test to read.
#[derive(Clone, Default)]
struct Captured(Arc<Mutex<Vec<u8>>>);

impl Write for Captured {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl Captured {
    fn text(&self) -> String {
        String::from_utf8_lossy(&self.0.lock().unwrap()).into_owned()
    }
}

fn io(err: &Captured) -> Io {
    Io {
        stdout: Box::new(std::io::sink()),
        stderr: SharedWriter::new(err.clone()),
        stdin: Box::new(std::io::empty()),
    }
}

fn kilnpp() -> String {
    Path::new(env!("CARGO_BIN_EXE_kiln")).with_file_name("kiln++").to_string_lossy().into_owned()
}

/// A fresh directory holding `files`.
fn scratch(test: &str, files: &[(&str, &str)]) -> PathBuf {
    let d = std::env::temp_dir().join(format!("kiln-eval-session-{}-{test}", std::process::id()));
    for (name, src) in files {
        let p = d.join(name);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, src).unwrap();
    }
    d
}

/// Op 1 returns 7.
struct Seven;

impl HostCalls for Seven {
    fn call(
        &mut self,
        op: u32,
        _ints: &[u64],
        _bytes: &[u8],
        _out: &mut HostOut<'_>,
    ) -> Result<u64, HostError> {
        if op == 1 { Ok(7) } else { Err(HostError::new("unknown op")) }
    }
}

#[test]
fn opened_from_a_command_line() {
    let d = scratch(
        "open",
        &[
            ("inc/lib.h", "#include <vector>\nconstexpr int scale = SCALE;\n"),
            (
                "t.cpp",
                "#include \"lib.h\"\n\
                 consteval int entry() {\n\
                   std::vector<int> v;\n\
                   for (int i = 0; i < 1000; ++i) v.push_back(i * scale);\n\
                   return v.back() + (int)__builtin_kiln_host_call(1, nullptr, 0, nullptr, 0, nullptr, 0, nullptr, 0);\n\
                 }\n",
            ),
        ],
    );
    let err = Captured::default();
    let t = d.join("t.cpp").to_string_lossy().into_owned();
    let inc = format!("-I{}", d.join("inc").display());
    let cl = ["-std=c++26".to_owned(), inc, "-DSCALE=3".to_owned(), t];
    let got = with_eval_session(&kilnpp(), &cl, io(&err), |fe| {
        let mut fe = fe.expect("session opens");
        assert!(fe.main.is_some());
        let e = fe.session.entry_named("entry").expect("entry");
        let mut out = Vec::new();
        for _ in 0..3 {
            let r = fe.session.run(e, &mut Seven, RunBudget::PROGRAM, &CancelToken::new());
            assert!(r.diagnostics.is_empty(), "{:?}", r.diagnostics);
            out.push((r.result, r.steps));
        }
        out
    });
    assert_eq!(err.text(), "");
    assert_eq!(got[0].0, Ok(RunValue::Signed(999 * 3 + 7)));
    assert!(got.iter().all(|g| *g == got[0]), "{got:?}");
    std::fs::remove_dir_all(&d).unwrap();
}

#[test]
fn a_translation_unit_with_errors_is_refused() {
    let d = scratch("refused", &[("t.cpp", "int x = ;\n")]);
    let err = Captured::default();
    let cl = ["-std=c++26".to_owned(), d.join("t.cpp").to_string_lossy().into_owned()];
    let got = with_eval_session(&kilnpp(), &cl, io(&err), |fe| fe.map(drop).err());
    assert_eq!(got, Some(SessionRefused::TuHasErrors { errors: 1 }));
    let text = err.text();
    assert!(text.contains("t.cpp:1:9: error: expected expression"), "{text}");
    assert!(
        text.contains(
            "error: cannot open an evaluation session: the translation unit has 1 error(s)"
        ),
        "{text}"
    );
    std::fs::remove_dir_all(&d).unwrap();
}

#[test]
fn a_command_line_without_one_source_is_refused() {
    let err = Captured::default();
    let got =
        with_eval_session(&kilnpp(), &["-std=c++26".to_owned()], io(&err), |fe| fe.map(drop).err());
    assert_eq!(got, Some(SessionRefused::CommandLine));
    let missing = ["no-such-file.cpp".to_owned()];
    let got = with_eval_session(&kilnpp(), &missing, io(&err), |fe| fe.map(drop).err());
    assert_eq!(got, Some(SessionRefused::CommandLine));
    assert!(err.text().contains("no-such-file.cpp"), "{}", err.text());
}

#[test]
fn reflection_in_runs() {
    let d = scratch(
        "meta",
        &[(
            "t.cpp",
            "#include \"kiln_meta.h\"\n\
             using namespace std::meta;\n\
             struct A;\n\
             struct B { int x; int y; long z; };\n\
             consteval int members() {\n\
               return (int)nonstatic_data_members_of(^^B, access_context::unchecked()).size();\n\
             }\n\
             consteval int define_in_run() {\n\
               define_aggregate(^^A, {data_member_spec(^^int, {.name = \"i\"})});\n\
               return 1;\n\
             }\n",
        )],
    );
    let support = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/support");
    let err = Captured::default();
    let cl = [
        "-std=c++26".to_owned(),
        "-freflection".to_owned(),
        format!("-I{}", support.display()),
        d.join("t.cpp").to_string_lossy().into_owned(),
    ];
    with_eval_session(&kilnpp(), &cl, io(&err), |fe| {
        let mut fe = fe.unwrap_or_else(|e| panic!("{e:?}: {}", err.text()));
        let m = fe.session.entry_named("members").unwrap();
        let def = fe.session.entry_named("define_in_run").unwrap();
        let mut first = None;
        for _ in 0..3 {
            let r = fe.session.run(m, &mut Seven, RunBudget::PROGRAM, &CancelToken::new());
            assert_eq!(r.result, Ok(RunValue::Signed(3)), "{:?}", r.diagnostics);
            // A run cannot inject a declaration ([expr.const]): every run
            // fails the same way, and nothing it tried is kept.
            let r = fe.session.run(def, &mut Seven, RunBudget::PROGRAM, &CancelToken::new());
            assert!(r.result.is_err());
            let msgs: Vec<String> = r.diagnostics.iter().map(|d| d.message.clone()).collect();
            assert!(!msgs.is_empty());
            assert_eq!(first.get_or_insert_with(|| msgs.clone()), &msgs);
        }
    });
    assert_eq!(err.text(), "");
    std::fs::remove_dir_all(&d).unwrap();
}

/// `VmRSS` and `VmHWM` of this process in KiB (Linux; 0 elsewhere).
fn rss_kib() -> (u64, u64) {
    let s = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    let field = |k: &str| {
        s.lines()
            .find_map(|l| l.strip_prefix(k))
            .and_then(|v| v.trim().trim_end_matches("kB").trim().parse().ok())
            .unwrap_or(0)
    };
    (field("VmRSS:"), field("VmHWM:"))
}

/// Op 1 returns its call count.
#[derive(Default)]
struct Count(u64);

impl HostCalls for Count {
    fn call(
        &mut self,
        _op: u32,
        _ints: &[u64],
        _bytes: &[u8],
        _out: &mut HostOut<'_>,
    ) -> Result<u64, HostError> {
        self.0 += 1;
        Ok(self.0)
    }
}

const TRANSFORM: &str = "using u64 = unsigned long long;\n\
constexpr u64 hc(unsigned op) {\n\
  return __builtin_kiln_host_call(op, nullptr, 0, nullptr, 0, nullptr, 0, nullptr, 0);\n\
}\n\
consteval void empty() {}\n\
consteval int one() { return 1; }\n\
consteval u64 hosts() { u64 s = 0; for (int i = 0; i < 100; ++i) s += hc(1); return s; }\n\
consteval int spin() { for (;;) {} return 0; }\n";

/// `runs` runs of `name` with a fresh handler each; their total time.
fn time_runs(session: &mut EvalSession<'_, '_>, name: &str, runs: u32) -> Duration {
    let entry = session.entry_named(name).unwrap();
    let token = CancelToken::new();
    let start = Instant::now();
    for _ in 0..runs {
        let report = session.run(entry, &mut Count::default(), RunBudget::PROGRAM, &token);
        assert!(report.result.is_ok());
    }
    start.elapsed()
}

/// From `cancel()` on another thread to the end of a `for (;;) {}` run.
fn cancel_latency(session: &mut EvalSession<'_, '_>) -> Duration {
    let spin = session.entry_named("spin").unwrap();
    let token = CancelToken::new();
    std::thread::scope(|scope| {
        let shared = &token;
        let canceller = scope.spawn(move || {
            std::thread::sleep(Duration::from_millis(50));
            shared.cancel();
            Instant::now()
        });
        let report = session.run(spin, &mut Count::default(), RunBudget::PROGRAM, &token);
        let done = Instant::now();
        assert_eq!(report.result, Err(kiln_parse::RunError::Cancelled));
        done.saturating_duration_since(canceller.join().unwrap())
    })
}

/// Runs the 10^6-element vector entry up to 1000 times (at most 10
/// minutes): is `kept()` after the last run `kept()` after run 2?
fn vector_flat(session: &mut EvalSession<'_, '_>) {
    let Ok(vec) = session.entry_named("vec") else { return };
    let token = CancelToken::new();
    let start = Instant::now();
    let first = session.run(vec, &mut Count::default(), RunBudget::PROGRAM, &token);
    println!(
        "1e6-element vector: {:?} per run, {} steps, peak {} KiB ({:?})",
        start.elapsed(),
        first.steps,
        first.peak_heap / 1024,
        first.result
    );
    session.run(vec, &mut Count::default(), RunBudget::PROGRAM, &token);
    let second = session.kept();
    let (rss_before, _) = rss_kib();
    let start = Instant::now();
    let mut runs = 2;
    while runs < 1000 && start.elapsed().as_secs() < 600 {
        session.run(vec, &mut Count::default(), RunBudget::PROGRAM, &token);
        runs += 1;
    }
    let (rss_after, hwm) = rss_kib();
    println!(
        "kept after run {runs} == after run 2: {} ({:?}); RSS {rss_before} -> {rss_after} KiB, \
         peak {hwm} KiB",
        session.kept() == second,
        session.kept()
    );
}

/// The per-run numbers of one session.
fn measure_runs(session: &mut EvalSession<'_, '_>) {
    let start = Instant::now();
    let empty = session.entry_named("empty").unwrap();
    let report = session.run(empty, &mut Count::default(), RunBudget::PROGRAM, &CancelToken::new());
    println!(
        "first empty run (open to first instruction): {:?} ({:?})",
        start.elapsed(),
        report.result
    );
    let runs = 100_000u32;
    println!("empty run: {:?} each", time_runs(session, "empty", runs) / runs);
    let trivial = time_runs(session, "one", runs);
    println!("trivial entry: {:.0} runs/s", f64::from(runs) / trivial.as_secs_f64());
    time_runs(session, "hosts", 2);
    let kept = session.kept();
    let hosts = time_runs(session, "hosts", 50_000);
    println!(
        "50,000 runs x 100 host calls: {hosts:?} ({:?} per run); kept flat: {}",
        hosts / 50_000,
        session.kept() == kept
    );
    println!("cancel latency (for (;;) {{}}): {:?}", cancel_latency(session));
    vector_flat(session);
}

/// The numbers of spec/eval-session.md §10 (a release build:
/// `cargo test --release -p kiln_driver --test eval_session -- --ignored
/// --nocapture measure`).
#[test]
#[ignore = "measurement: run in a release build with --ignored --nocapture"]
fn measure() {
    let vector = "consteval unsigned long vec() { std::vector<int> v; \
                  for (int i = 0; i < 1000000; ++i) v.push_back(i); return v.size(); }\n";
    for (label, pre) in
        [("small transform", ""), ("<bits/stdc++.h> + transform", "#include <bits/stdc++.h>\n")]
    {
        let body =
            if pre.is_empty() { TRANSFORM.to_owned() } else { format!("{pre}{TRANSFORM}{vector}") };
        let dir = scratch("measure", &[("t.cpp", &body)]);
        let err = Captured::default();
        let cl = [
            "-std=c++26".to_owned(),
            "-O0".to_owned(),
            dir.join("t.cpp").to_string_lossy().into_owned(),
        ];
        let (rss_start, _) = rss_kib();
        let start = Instant::now();
        with_eval_session(&kilnpp(), &cl, io(&err), |fe| {
            let opened = start.elapsed();
            let mut fe = fe.unwrap_or_else(|e| panic!("{e:?}: {}", err.text()));
            let (rss_open, _) = rss_kib();
            let kept = fe.session.kept();
            println!("== {label}");
            println!(
                "parse + open: {opened:?}; RSS +{} KiB; kept {kept:?} (total {} KiB)",
                rss_open.saturating_sub(rss_start),
                kept.total() / 1024
            );
            measure_runs(&mut fe.session);
        });
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
