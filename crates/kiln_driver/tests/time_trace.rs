//! `-ftime-trace[=<path>]`, `-ftime-trace-granularity=<n>` and
//! `-ftime-trace-verbose` (spec/driver.md "Time trace", WP-1115): the
//! trace is written where clang writes it, is a well-formed Chrome trace
//! (`{"traceEvents":[...],"beginningOfTime":N}`, metadata, properly nested
//! complete events, `Total` events), and carries clang's event names plus
//! the Oven's. Paths are compared with `clang++-19`'s where it is
//! installed. Tests that run `llc` skip without `llc-19` / `clang++-19`.
#![forbid(unsafe_code)]

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use kiln_source::trace::Json;

fn kilnpp() -> PathBuf {
    Path::new(env!("CARGO_BIN_EXE_kiln")).with_file_name("kiln++")
}

fn have(tool: &str) -> bool {
    Command::new(tool).arg("--version").stdout(Stdio::null()).stderr(Stdio::null()).status().is_ok()
}

fn tools() -> bool {
    let ok = have("llc-19") && have("clang++-19");
    if !ok {
        eprintln!("skipping: llc-19 / clang++-19 not found");
    }
    ok
}

/// A header with a class template, a `consteval` function called from a
/// `constexpr` variable's initializer, and an instantiation of the
/// template and its member function.
const HEADER: &str = "template<class T> struct Box { T v; T get() const { return v; } };\n";
const MAIN: &str = "#include \"h.h\"\n\
                    consteval int fib(int n) { return n < 2 ? n : fib(n - 1) + fib(n - 2); }\n\
                    constexpr int k = fib(12);\n\
                    static_assert(k == 144);\n\
                    int main() { Box<int> b{k}; return b.get(); }\n";

fn setup(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("kiln-time-trace-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(d.join("out")).unwrap();
    std::fs::create_dir_all(d.join("tr")).unwrap();
    std::fs::write(d.join("h.h"), HEADER).unwrap();
    std::fs::write(d.join("t.cpp"), MAIN).unwrap();
    d
}

fn run(cc: &Path, dir: &Path, args: &[&str]) -> Output {
    let o = Command::new(cc).args(args).current_dir(dir).output().unwrap();
    assert!(
        o.status.success(),
        "{} {args:?}: {}",
        cc.display(),
        String::from_utf8_lossy(&o.stderr)
    );
    o
}

/// The `.json` files under `dir` (one level of subdirectories), relative.
fn traces(dir: &Path) -> BTreeSet<String> {
    let mut s = BTreeSet::new();
    let mut walk = |d: &Path, prefix: &str| {
        for e in std::fs::read_dir(d).unwrap().flatten() {
            let n = e.file_name().to_string_lossy().into_owned();
            if n.ends_with(".json") {
                s.insert(format!("{prefix}{n}"));
            }
        }
    };
    walk(dir, "");
    walk(&dir.join("out"), "out/");
    walk(&dir.join("tr"), "tr/");
    s
}

fn clear(dir: &Path) {
    for t in traces(dir) {
        std::fs::remove_file(dir.join(t)).unwrap();
    }
}

/// Every command line writes its trace where clang does: `-c` next to the
/// object, `-o` renamed, `=<dir>`, `=<file>`, linking (`<-o>-<input>`),
/// `-E` to standard output (`-.json`) and to a file.
#[test]
fn written_at_clangs_path() {
    if !tools() {
        return;
    }
    let cases: &[(&[&str], &str)] = &[
        (&["-c", "-ftime-trace", "t.cpp"], "t.json"),
        (&["-c", "-ftime-trace", "t.cpp", "-o", "out/o1.o"], "out/o1.json"),
        (&["-c", "-ftime-trace=tr", "t.cpp", "-o", "out/o1.o"], "tr/o1.json"),
        (&["-c", "-ftime-trace=out/f.json", "t.cpp"], "out/f.json"),
        (&["-ftime-trace", "t.cpp", "-o", "out/app"], "out/app-t.json"),
        (&["-E", "-ftime-trace", "t.cpp"], "-.json"),
        (&["-E", "-ftime-trace", "t.cpp", "-o", "out/t.ii"], "out/t.json"),
    ];
    let d = setup("paths");
    let clang = have("clang++-19");
    for (args, want) in cases {
        let mut a = vec!["-std=c++20"];
        a.extend_from_slice(args);
        for cc in [Some(kilnpp()), clang.then(|| PathBuf::from("clang++-19"))].into_iter().flatten()
        {
            clear(&d);
            run(&cc, &d, &a);
            assert_eq!(traces(&d), BTreeSet::from([(*want).to_owned()]), "{} {a:?}", cc.display());
            let text = std::fs::read_to_string(d.join(want)).unwrap();
            check_structure(&Json::parse(&text).unwrap());
        }
    }
}

/// An event's `name`, `ph`, `ts`, `dur`, `tid` and `args`.
struct Ev<'a> {
    name: &'a str,
    ph: &'a str,
    ts: u64,
    dur: u64,
    tid: u64,
    args: Option<&'a Json>,
}

impl Ev<'_> {
    fn arg(&self, k: &str) -> Option<&Json> {
        self.args.and_then(|a| a.get(k))
    }
    fn detail(&self) -> &str {
        self.arg("detail").and_then(Json::as_str).unwrap_or("")
    }
}

fn events(j: &Json) -> Vec<Ev<'_>> {
    let num = |e: &Json, k: &str| e.get(k).and_then(Json::as_u64).unwrap_or(0);
    j.get("traceEvents")
        .and_then(Json::as_array)
        .unwrap()
        .iter()
        .map(|e| Ev {
            name: e.get("name").and_then(Json::as_str).unwrap(),
            ph: e.get("ph").and_then(Json::as_str).unwrap(),
            ts: num(e, "ts"),
            dur: num(e, "dur"),
            tid: num(e, "tid"),
            args: e.get("args"),
        })
        .collect()
}

/// What Perfetto / chrome://tracing need, and what clang writes: the two
/// top-level members; every event with `pid`, `tid`, `ph`, `ts`, `name`;
/// complete events with `dur`, properly nested on the main thread; `b`/`e`
/// pairs with `cat` and `id`; the `process_name` and `thread_name`
/// metadata; `Total` events on their own threads with `count` and
/// `avg ms`.
fn check_structure(j: &Json) {
    let Json::Obj(top) = j else { panic!("not an object") };
    let keys: Vec<&str> = top.iter().map(|(k, _)| k.as_str()).collect();
    assert_eq!(keys, ["traceEvents", "beginningOfTime"]);
    assert!(j.get("beginningOfTime").and_then(Json::as_u64).is_some());
    let raw = j.get("traceEvents").and_then(Json::as_array).unwrap();
    for e in raw {
        for k in ["pid", "tid", "ph", "ts", "name"] {
            assert!(e.get(k).is_some(), "{e:?} lacks {k}");
        }
        match e.get("ph").and_then(Json::as_str).unwrap() {
            "X" => assert!(e.get("dur").and_then(Json::as_u64).is_some(), "{e:?}"),
            "b" | "e" => assert!(e.get("cat").is_some() && e.get("id").is_some(), "{e:?}"),
            "M" => {}
            p => panic!("unexpected phase {p}"),
        }
    }
    let ev = events(j);
    let meta: Vec<&str> = ev.iter().filter(|e| e.ph == "M").map(|e| e.name).collect();
    assert_eq!(meta, ["process_name", "thread_name"]);
    let main = ev.iter().find(|e| e.ph == "M").unwrap().tid;
    // Complete events on the main thread nest: sorted by start (longest
    // first), each starts after the enclosing one ends or ends within it.
    let mut xs: Vec<&Ev> = ev.iter().filter(|e| e.ph == "X" && e.tid == main).collect();
    xs.sort_by_key(|e| (e.ts, std::cmp::Reverse(e.dur)));
    let mut open: Vec<u64> = Vec::new();
    for e in xs {
        while open.last().is_some_and(|&end| end <= e.ts) {
            open.pop();
        }
        if let Some(&end) = open.last() {
            assert!(
                e.ts + e.dur <= end,
                "{} [{}, +{}] crosses an enclosing event",
                e.name,
                e.ts,
                e.dur
            );
        }
        open.push(e.ts + e.dur);
    }
    let (b, en) =
        (ev.iter().filter(|e| e.ph == "b").count(), ev.iter().filter(|e| e.ph == "e").count());
    assert_eq!(b, en, "unbalanced async events");
    for t in ev.iter().filter(|e| e.name.starts_with("Total ")) {
        assert!(t.tid > main && t.ph == "X", "{}", t.name);
        assert!(t.arg("count").and_then(Json::as_u64).is_some_and(|c| c > 0), "{}", t.name);
        assert!(t.arg("avg ms").and_then(Json::as_u64).is_some(), "{}", t.name);
    }
}

fn trace_of(name: &str, extra: &[&str]) -> Json {
    let d = setup(name);
    let mut a = vec!["-std=c++20", "-c", "-ftime-trace", "t.cpp"];
    a.extend_from_slice(extra);
    run(&kilnpp(), &d, &a);
    let j = Json::parse(&std::fs::read_to_string(d.join("t.json")).unwrap()).unwrap();
    check_structure(&j);
    j
}

/// clang's frontend and backend events, and the Oven's top-level
/// evaluations ([expr.const]/22: the immediate invocation `fib(12)`, the
/// initializer of `k`, the `static_assert` condition), with clang's
/// details; `-ftime-trace-verbose` adds the per-call `ConstevalCall`
/// events and the instantiations' `file` / `line`.
#[test]
fn event_names_and_details() {
    if !tools() {
        return;
    }
    let j = trace_of("names", &["-ftime-trace-granularity=0"]);
    let ev = events(&j);
    let has = |name: &str, detail: &str| ev.iter().any(|e| e.name == name && e.detail() == detail);
    for (name, detail) in [
        ("ExecuteCompiler", ""),
        ("Frontend", ""),
        ("Source", "./h.h"),
        ("ParseClass", "Box"),
        ("ParseFunctionDefinition", "fib"),
        ("ParseDeclarationOrFunctionDefinition", "t.cpp:3:1"),
        ("InstantiateClass", "Box<int>"),
        ("InstantiateFunction", "Box<int>::get"),
        ("PerformPendingInstantiations", ""),
        ("EvaluateAsConstantExpr", "<t.cpp:3:19, col:25>"),
        ("EvaluateAsInitializer", "k"),
        ("EvaluateAsRValue", "<t.cpp:4:15, col:20>"),
        ("OvenCompile", "fib"),
        ("CodeGen Function", "main"),
        ("CodeGen Function", "Box<int>::get"),
        ("Backend", ""),
        ("CodeGenPasses", ""),
        ("Total ExecuteCompiler", ""),
        ("Total InstantiateClass", ""),
    ] {
        assert!(has(name, detail), "no {name} event with detail {detail:?}");
    }
    assert!(
        !ev.iter().any(|e| e.name == "ConstevalCall"),
        "per-call events need -ftime-trace-verbose"
    );
    let inst = ev.iter().find(|e| e.name == "InstantiateClass").unwrap();
    assert!(inst.arg("file").is_none());

    let j = trace_of("verbose", &["-ftime-trace-granularity=0", "-ftime-trace-verbose"]);
    let ev = events(&j);
    // Memoized: each `fib(n)` runs once, `fib(12)` .. `fib(0)`.
    assert!(ev.iter().filter(|e| e.name == "ConstevalCall" && e.detail() == "fib").count() >= 13);
    let inst = ev.iter().find(|e| e.name == "InstantiateClass").unwrap();
    assert_eq!(inst.arg("file").and_then(Json::as_str), Some("./h.h"));
    assert_eq!(inst.arg("line").and_then(Json::as_u64), Some(1));
    let eval = ev.iter().find(|e| e.name == "EvaluateAsConstantExpr").unwrap();
    assert_eq!(eval.arg("kind").and_then(Json::as_str), Some("consteval"));
    assert!(eval.arg("steps").and_then(Json::as_u64).is_some_and(|s| s > 0));
}

/// Events shorter than the granularity are dropped; the `Total` events
/// still count them (clang `TimeTraceProfiler`).
#[test]
fn granularity_drops_short_events() {
    if !tools() {
        return;
    }
    let j = trace_of("gran", &["-ftime-trace-granularity=100000000"]);
    let ev = events(&j);
    assert!(
        ev.iter().all(|e| e.ph == "M" || e.name.starts_with("Total ")),
        "an event outlived 100 s"
    );
    for t in ["Total ParseClass", "Total InstantiateClass", "Total EvaluateAsInitializer"] {
        let e = ev.iter().find(|e| e.name == t).unwrap_or_else(|| panic!("no {t}"));
        assert_eq!(e.arg("count").and_then(Json::as_u64), Some(1), "{t}");
    }
}

/// Without `-ftime-trace` nothing is written and nothing is said.
#[test]
fn no_flag_no_trace() {
    if !tools() {
        return;
    }
    let d = setup("off");
    let o = run(&kilnpp(), &d, &["-std=c++20", "-c", "t.cpp"]);
    assert_eq!(String::from_utf8_lossy(&o.stderr), "");
    assert!(traces(&d).is_empty());
}

/// A call the VM unwinds without returning (a failed
/// `__builtin_constant_p` trial) ends when the next call at its depth
/// begins, not when its caller returns: the trials' events are disjoint
/// siblings, one per trial (spec/oven.md "Trace hook").
#[test]
fn unwound_calls_end_at_the_next_call() {
    let d = setup("unwind");
    std::fs::write(
        d.join("u.cpp"),
        "constexpr int helper(int i) { int z = 0; return i / z; }\n\
         constexpr int trials(int n) {\n\
         \x20 int r = 0;\n\
         \x20 for (int i = 0; i < n; ++i) r += __builtin_constant_p(helper(i));\n\
         \x20 return r;\n\
         }\n\
         static_assert(trials(200) == 0);\n",
    )
    .unwrap();
    let args = [
        "-std=c++20",
        "-S",
        "-emit-llvm",
        "-ftime-trace",
        "-ftime-trace-verbose",
        "-ftime-trace-granularity=0",
    ];
    run(&kilnpp(), &d, &[&args[..], &["u.cpp"]].concat());
    let j = Json::parse(&std::fs::read_to_string(d.join("u.json")).unwrap()).unwrap();
    check_structure(&j);
    let mut calls: Vec<Ev> = events(&j)
        .into_iter()
        .filter(|e| e.name == "ConstexprCall" && e.detail() == "helper")
        .collect();
    assert_eq!(calls.len(), 200);
    calls.sort_by_key(|e| e.ts);
    for w in calls.windows(2) {
        assert!(
            w[0].ts + w[0].dur <= w[1].ts,
            "trial calls overlap: [{}, +{}] and {}",
            w[0].ts,
            w[0].dur,
            w[1].ts
        );
    }
}
