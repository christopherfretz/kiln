//! WP-1995: retained evaluation sessions (spec/eval-session.md) through
//! the library interface a tooling client uses: a translation unit is
//! parsed with `parse_preprocessed_retained`, opened as an
//! [`EvalSession`] and its consteval entries run many times, each run
//! isolated, budgeted, cancellable, with its own host-call handler and
//! its own diagnostics.
#![forbid(unsafe_code)]

use std::sync::Arc;
use std::time::{Duration, Instant};

use kiln_diag::{DiagEngine, DiagOptions, Diagnostic, MemorySink};
use kiln_parse::{
    CancelToken, ConstexprLimits, EntryError, EvalSession, HostCalls, HostError, HostOut,
    OpenError, ParseOptions, RunBudget, RunError, RunReport, RunValue, parse_preprocessed_retained,
};
use kiln_pp::{ClangFeatures, ModuleOptions, PpOptions, Preprocessor};
use kiln_sema::target::{Arch, Target};
use kiln_source::SourceManager;
use kiln_source::symbol::Interner;

/// The wrapper every test source starts with (as in `host_calls.rs`).
const PRE: &str = "using u64 = unsigned long long;\n\
constexpr u64 hc(unsigned op, const u64* in = nullptr, unsigned n_in = 0, \
const char* bin = nullptr, unsigned n_bin = 0, u64* out = nullptr, unsigned co = 0, \
char* bout = nullptr, unsigned cb = 0) {\n\
return __builtin_kiln_host_call(op, in, n_in, bin, n_bin, out, co, bout, cb);\n\
}\n";

/// A run's handler: op 1 returns `next` and counts up, op 2 doubles its
/// first integer, op 3 fails.
#[derive(Default)]
struct Counter {
    next: u64,
    seen: u64,
}

impl Counter {
    fn from(next: u64) -> Self {
        Counter { next, seen: 0 }
    }
}

impl HostCalls for Counter {
    fn call(
        &mut self,
        op: u32,
        ints: &[u64],
        _bytes: &[u8],
        _out: &mut HostOut<'_>,
    ) -> Result<u64, HostError> {
        self.seen += 1;
        match op {
            1 => {
                self.next += 1;
                Ok(self.next - 1)
            }
            2 => Ok(ints.first().copied().unwrap_or(0) * 2),
            3 => Err(HostError::new("boom")),
            _ => Err(HostError::new(format!("unknown op {op}"))),
        }
    }
}

/// The translation unit's handler: every call fails, so a run that
/// reached it instead of its own would show.
struct TuHandler;

impl HostCalls for TuHandler {
    fn call(
        &mut self,
        _op: u32,
        _ints: &[u64],
        _bytes: &[u8],
        _out: &mut HostOut<'_>,
    ) -> Result<u64, HostError> {
        Err(HostError::new("the translation unit's handler"))
    }
}

/// Each diagnostic and note of `ds` as `L:C message`.
fn render(sm: &SourceManager, ds: &[Diagnostic]) -> Vec<String> {
    fn one(sm: &SourceManager, d: &Diagnostic, out: &mut Vec<String>) {
        let pos = sm
            .line_col(d.span.lo)
            .map_or_else(|| "-".to_owned(), |lc| format!("{}:{}", lc.line, lc.col));
        out.push(format!("{pos} {}", d.message));
        for n in &d.notes {
            one(sm, n, out);
        }
    }
    let mut out = Vec::new();
    for d in ds {
        one(sm, d, &mut out);
    }
    out
}

/// Parses `PRE` + `src` and calls `f` with the session (or why it was
/// refused), the source manager and the translation unit's diagnostics.
fn with_session<R>(
    src: &str,
    f: impl for<'a, 'sm> FnOnce(
        Result<EvalSession<'a, 'sm>, OpenError>,
        &'sm SourceManager,
        &MemorySink,
    ) -> R,
) -> R {
    let sm = SourceManager::new();
    let file = sm.add_virtual("t.cpp", format!("{PRE}{src}")).expect("location space");
    let interner = Arc::new(Interner::new());
    let sink = MemorySink::new();
    let opts = ParseOptions::default();
    let dopts = DiagOptions { clang_compat: opts.clang_compat, ..DiagOptions::default() };
    let engine = DiagEngine::new(&sm, &dopts, sink.clone());
    let mut ppo = PpOptions::new(opts.std);
    ppo.parser_pragmas = true;
    ppo.modules = Some(ModuleOptions::default());
    let mut pp = Preprocessor::new(&sm, &engine, &interner, ppo);
    let features = ClangFeatures::new("x86_64-unknown-linux-gnu", opts.std, &[])
        .expect("x86-64 features")
        .with_kiln_host_calls(true);
    pp.set_feature_queries(Box::new(features));
    pp.enter_main_file(file);
    let (sema, _pp) = parse_preprocessed_retained(
        pp,
        Arc::clone(&interner),
        &engine,
        Target::for_arch(Arch::X86_64),
        &opts,
        ConstexprLimits::default(),
        Some(Box::new(TuHandler)),
    );
    f(EvalSession::open(sema), &sm, &sink)
}

/// [`with_session`] for a translation unit that must open cleanly.
fn session<R>(
    src: &str,
    f: impl for<'a, 'sm> FnOnce(&mut EvalSession<'a, 'sm>, &'sm SourceManager) -> R,
) -> R {
    with_session(src, |s, sm, sink| {
        let tu = render(sm, &sink.take());
        assert!(tu.is_empty(), "translation unit diagnostics:\n{}", tu.join("\n"));
        let mut s = s.expect("session opens");
        let r = f(&mut s, sm);
        let late = render(sm, &sink.take());
        assert!(
            late.is_empty(),
            "runs' diagnostics reached the engine's sink:\n{}",
            late.join("\n")
        );
        r
    })
}

/// Runs `name` with a fresh [`Counter`] under `budget`.
fn run_named(s: &mut EvalSession<'_, '_>, name: &str, budget: RunBudget) -> RunReport {
    let e = s.entry_named(name).unwrap_or_else(|e| panic!("entry {name}: {e}"));
    s.run(e, &mut Counter::default(), budget, &CancelToken::new())
}

fn run(s: &mut EvalSession<'_, '_>, name: &str) -> RunReport {
    run_named(s, name, RunBudget::PROGRAM)
}

#[track_caller]
fn ok(r: &RunReport) -> RunValue {
    assert!(r.diagnostics.is_empty(), "unexpected diagnostics: {:?}", r.diagnostics);
    match &r.result {
        Ok(v) => *v,
        Err(e) => panic!("run failed: {e:?}"),
    }
}

const FIB: &str = "constexpr int fib(int n) { return n < 2 ? n : fib(n - 1) + fib(n - 2); }\n\
consteval int fib_entry() { return fib(25); }\n\
consteval int small_fib() { return fib(12); }\n";

#[test]
fn values_of_each_return_kind() {
    session(
        "consteval void v() {}\n\
         consteval bool b() { return true; }\n\
         consteval int neg() { return -7; }\n\
         consteval unsigned long long big() { return ~0ull; }\n\
         consteval __int128 wide() { return (__int128)1 << 100; }\n",
        |s, _| {
            assert_eq!(ok(&run(s, "v")), RunValue::Void);
            assert_eq!(ok(&run(s, "b")), RunValue::Bool(true));
            assert_eq!(ok(&run(s, "neg")), RunValue::Signed(-7));
            assert_eq!(ok(&run(s, "big")), RunValue::Unsigned(u128::from(u64::MAX)));
            assert_eq!(ok(&run(s, "wide")), RunValue::Signed(1 << 100));
        },
    );
}

#[test]
fn steps_do_not_depend_on_memoized_results() {
    session(FIB, |s, _| {
        let first = run(s, "fib_entry");
        assert_eq!(ok(&first), RunValue::Signed(75_025));
        assert!(first.steps > 100_000, "{} steps", first.steps);
        // Later runs hit the memoization cache and report the same cost.
        assert!(s.kept().memo_entries > 0);
        for _ in 0..999 {
            let r = run(s, "fib_entry");
            assert_eq!(ok(&r), RunValue::Signed(75_025));
            assert_eq!((r.steps, r.calls), (first.steps, first.calls));
        }
        // And so does a run with the cache emptied.
        assert!(s.clear_memo() > 0);
        assert_eq!(s.kept().memo_entries, 0);
        let cold = run(s, "fib_entry");
        assert_eq!(ok(&cold), RunValue::Signed(75_025));
        assert_eq!((cold.steps, cold.calls), (first.steps, first.calls));
    });
}

#[test]
fn step_budget_is_exact_fresh_and_used() {
    let steps = session(FIB, |s, _| run(s, "small_fib").steps);
    // A fresh session: one step short fails, the exact budget passes.
    session(FIB, |s, _| {
        let short = run_named(s, "small_fib", RunBudget::PROGRAM.with_steps(steps - 1));
        assert_eq!(short.result, Err(RunError::StepBudget), "{:?}", short.diagnostics);
        assert_eq!(short.diagnostics.len(), 1);
        let exact = run_named(s, "small_fib", RunBudget::PROGRAM.with_steps(steps));
        assert_eq!(ok(&exact), RunValue::Signed(144));
        assert_eq!(exact.steps, steps);
        // Used (every result now memoized): the same edge.
        for _ in 0..3 {
            let short = run_named(s, "small_fib", RunBudget::PROGRAM.with_steps(steps - 1));
            assert_eq!(short.result, Err(RunError::StepBudget));
            let exact = run_named(s, "small_fib", RunBudget::PROGRAM.with_steps(steps));
            assert_eq!(ok(&exact), RunValue::Signed(144));
        }
    });
}

const FAILURES: &str = "namespace std { struct exception { \
constexpr virtual ~exception() {} \
constexpr virtual const char* what() const noexcept { return \"std::exception\"; } }; }\n\
struct E : std::exception { constexpr const char* what() const noexcept override { return \"bad thing\"; } };\n\
constexpr int at(int i) { int a[2]{}; return a[i]; }\n\
consteval int good() { int s = 0; for (int i = 0; i < 100; ++i) s += i; return s; }\n\
consteval int bad() { return at(5); }\n\
consteval int spin() { for (;;) {} return 0; }\n\
consteval int big() { char* p = new char[1 << 20]; p[0] = 1; int r = p[0]; delete[] p; return r; }\n\
constexpr int rec(int n) { return n < 0 ? 0 : rec(n + 1) + 1; }\n\
consteval int deep() { return rec(0); }\n\
consteval u64 herr() { return hc(3); }\n\
consteval int thrown() { throw E{}; return 0; }\n\
consteval int leak() { new int(3); return 1; }\n\
constexpr int raise(int n) { if (n) throw n; return 0; }\n\
struct L { static constexpr int f() noexcept { return raise(1); } };\n\
consteval int nothrow_exit() { return L::f(); }\n";

#[test]
fn every_failure_kind_leaves_the_session_as_fresh() {
    let baseline = session(FAILURES, |s, _| run(s, "good").steps);
    session(FAILURES, |s, sm| {
        let tiny = RunBudget::PROGRAM;
        let cancelled = CancelToken::new();
        cancelled.cancel();
        let cases: [(&str, RunBudget, Option<&CancelToken>, RunError, &str); 9] = [
            ("bad", tiny, None, RunError::Failed, "cannot refer to element 5"),
            ("spin", tiny.with_steps(100_000), None, RunError::StepBudget, ""),
            ("big", tiny.with_bytes(4096), None, RunError::MemoryBudget, ""),
            ("deep", tiny.with_depth(64), None, RunError::DepthLimit, ""),
            ("spin", tiny, Some(&cancelled), RunError::Cancelled, "evaluation was cancelled"),
            ("herr", tiny, None, RunError::Host { message: "boom".into() }, "boom"),
            (
                "thrown",
                tiny,
                None,
                RunError::Uncaught { type_name: "E".into(), what: Some("bad thing".into()) },
                "",
            ),
            ("leak", tiny, None, RunError::Failed, "not deallocated"),
            ("nothrow_exit", tiny, None, RunError::Failed, ""),
        ];
        for (name, budget, cancel, want, needle) in cases {
            for _ in 0..2 {
                let e = s.entry_named(name).unwrap();
                let token = CancelToken::new();
                let r = s.run(e, &mut Counter::default(), budget, cancel.unwrap_or(&token));
                let shown = render(sm, &r.diagnostics);
                assert_eq!(r.result.as_ref().err(), Some(&want), "{name}: {shown:?}");
                assert!(
                    shown.first().is_some_and(|d| d.contains("is not a constant expression")),
                    "{name}: {shown:?}"
                );
                assert!(
                    shown.iter().any(|d| d.contains(needle)),
                    "{name}: {needle:?} in {shown:?}"
                );
                let after = run(s, "good");
                assert_eq!(ok(&after), RunValue::Signed(4950));
                assert_eq!(after.steps, baseline, "after {name}");
            }
        }
    });
}

#[test]
fn failure_diagnostics_have_call_stack_notes_and_repeat() {
    session(FAILURES, |s, sm| {
        let first = render(sm, &run(s, "bad").diagnostics);
        assert!(first.iter().any(|d| d.contains("in call to 'at(5)'")), "{first:?}");
        assert!(first[0].starts_with("9:"), "{first:?}");
        for _ in 0..3 {
            assert_eq!(render(sm, &run(s, "bad").diagnostics), first);
            assert!(run(s, "good").diagnostics.is_empty());
        }
    });
}

#[test]
fn cancelled_from_another_thread() {
    session(FAILURES, |s, _| {
        let e = s.entry_named("spin").unwrap();
        let token = CancelToken::new();
        let (report, latency) = std::thread::scope(|sc| {
            let t = &token;
            let canceller = sc.spawn(move || {
                std::thread::sleep(Duration::from_millis(100));
                t.cancel();
                Instant::now()
            });
            let r = s.run(e, &mut Counter::default(), RunBudget::PROGRAM, &token);
            let done = Instant::now();
            let at = canceller.join().unwrap();
            (r, done.saturating_duration_since(at))
        });
        assert_eq!(report.result, Err(RunError::Cancelled));
        assert!(report.steps > 0);
        // Unoptimized test builds are slow; release builds end within
        // milliseconds (spec/eval-session.md §10).
        assert!(latency < Duration::from_secs(2), "{latency:?}");
        // The token stays cancelled until reset.
        let again = s.run(e, &mut Counter::default(), RunBudget::PROGRAM, &token);
        assert_eq!(again.result, Err(RunError::Cancelled));
        token.reset();
        assert_eq!(ok(&run(s, "good")), RunValue::Signed(4950));
    });
}

#[test]
fn each_run_has_its_own_handler_and_no_memo() {
    session(
        "consteval u64 two() { u64 a = hc(1); u64 b = hc(1); return a * 1000 + b; }\n\
         consteval u64 doubled() { u64 in[1] = {21}; return hc(2, in, 1); }\n\
         template <class T> struct W { static consteval u64 get() { return hc(1); } };\n\
         static_assert(sizeof(W<int>) > 0);\n\
         using X = W<int>;\n",
        |s, _| {
            let e = s.entry_named("two").unwrap();
            for start in [0u64, 5, 5, 40] {
                let mut h = Counter::from(start);
                let r = s.run(e, &mut h, RunBudget::PROGRAM, &CancelToken::new());
                let want = u128::from(start * 1000 + start + 1);
                assert_eq!(ok(&r), RunValue::Unsigned(want));
                assert_eq!((r.host_calls, h.seen), (2, 2));
            }
            assert_eq!(ok(&run(s, "doubled")), RunValue::Unsigned(42));
            let e = s.entry_named("X::get").unwrap();
            for start in [3u64, 9] {
                let r =
                    s.run(e, &mut Counter::from(start), RunBudget::PROGRAM, &CancelToken::new());
                assert_eq!(ok(&r), RunValue::Unsigned(u128::from(start)));
            }
        },
    );
}

#[test]
fn constant_state_does_not_leak_between_runs() {
    session(
        "struct S { mutable int n = 0; };\n\
         constexpr S s{};\n\
         consteval int bump() { return ++s.n; }\n\
         consteval int alloc() { int* p = new int[4]{1, 2, 3, 4}; int r = p[3]; delete[] p; return r; }\n",
        |s, sm| {
            let first = run(s, "bump");
            assert_eq!(first.result, Err(RunError::Failed));
            let shown = render(sm, &first.diagnostics);
            for _ in 0..3 {
                let r = run(s, "bump");
                assert_eq!(r.result, Err(RunError::Failed));
                assert_eq!(render(sm, &r.diagnostics), shown);
                assert_eq!(ok(&run(s, "alloc")), RunValue::Signed(4));
            }
        },
    );
}

#[test]
fn instantiation_diagnostics_are_reported_by_every_run() {
    session(
        "[[deprecated(\"use new\")]] constexpr int old(int) { return 1; }\n\
         template <class T> struct W {\n\
           static consteval int run() { return old(T{}); }\n\
           static consteval int checked() { static_assert(hc(sizeof(T)) == 0); return 2; }\n\
         };\n\
         static_assert(sizeof(W<int>) > 0);\n\
         using X = W<int>;\n",
        |s, sm| {
            // The definition is instantiated by the first run; its warning
            // is that run's, and every later run reports it again.
            let first = run(s, "X::run");
            assert!(first.result.is_ok());
            let shown = render(sm, &first.diagnostics);
            assert!(shown.iter().any(|d| d.contains("'old' is deprecated")), "{shown:?}");
            for _ in 0..3 {
                let r = run(s, "X::run");
                assert_eq!(r.result, first.result);
                assert_eq!(render(sm, &r.diagnostics), shown);
            }
            // An instantiation that fails (a nested evaluation's host
            // call is refused) fails every run the same way.
            let bad = run(s, "X::checked");
            assert_eq!(bad.result, Err(RunError::Failed), "{:?}", render(sm, &bad.diagnostics));
            let shown = render(sm, &bad.diagnostics);
            assert!(shown.iter().any(|d| d.contains("host call in semantic work")), "{shown:?}");
            for _ in 0..3 {
                let r = run(s, "X::checked");
                assert_eq!(r.result, bad.result);
                assert_eq!(render(sm, &r.diagnostics), shown);
            }
            assert_eq!(run(s, "X::run").result, Ok(RunValue::Signed(1)));
        },
    );
}

#[test]
fn kept_memory_is_flat() {
    session(
        "consteval int small() { int* p = new int[1000]; for (int i = 0; i < 1000; ++i) p[i] = i; \
         int r = p[999]; delete[] p; return r; }\n\
         consteval int large() { int* p = new int[1000000]; for (int i = 0; i < 1000000; ++i) p[i] = i; \
         int r = p[999999]; delete[] p; return r; }\n\
         consteval u64 calls() { u64 s = 0; for (int i = 0; i < 10; ++i) s += hc(1); return s; }\n",
        |s, _| {
            for name in ["small", "calls"] {
                ok(&run(s, name));
                ok(&run(s, name));
                let second = s.kept();
                for _ in 0..1000 {
                    ok(&run(s, name));
                }
                assert_eq!(s.kept(), second, "{name}");
            }
            let r = run(s, "large");
            assert_eq!(ok(&r), RunValue::Signed(999_999));
            assert!(r.peak_heap >= 4_000_000, "{}", r.peak_heap);
            ok(&run(s, "large"));
            let second = s.kept();
            ok(&run(s, "large"));
            ok(&run(s, "large"));
            assert_eq!(s.kept(), second);
            // The run's heap was released, not kept.
            assert!(second.oven < 4_000_000, "{second:?}");
        },
    );
}

#[test]
fn entry_lookup_errors() {
    session(
        "consteval int ov() { return 1; }\n\
         consteval int ov(int) { return 2; }\n\
         constexpr int not_consteval() { return 3; }\n\
         consteval int params(int x) { return x; }\n\
         consteval double real() { return 1.0; }\n\
         int var;\n\
         template <class T> consteval int tmpl() { return 4; }\n\
         consteval int undefined();\n\
         namespace n { struct S { static consteval int f() { return 5; } \
           consteval int g() const { return 6; } }; namespace in { consteval int h() { return 7; } } }\n\
         namespace alias = n::in;\n\
         using T = n::S;\n",
        |s, _| {
            let err = |s: &EvalSession<'_, '_>, name: &str| s.entry_named(name).unwrap_err();
            assert_eq!(err(s, "missing"), EntryError::NotFound { name: "missing".into() });
            assert!(matches!(err(s, "n::missing"), EntryError::NotFound { .. }));
            assert!(matches!(err(s, "nowhere::f"), EntryError::NotFound { .. }));
            assert_eq!(err(s, "ov"), EntryError::Overloaded { name: "ov".into(), count: 2 });
            assert_eq!(err(s, "not_consteval"), EntryError::NotConsteval);
            assert_eq!(err(s, "params"), EntryError::HasParameters);
            assert_eq!(err(s, "real"), EntryError::WrongReturnType);
            assert_eq!(err(s, "var"), EntryError::NotAFunction);
            assert_eq!(err(s, "tmpl"), EntryError::NotAFunction);
            assert_eq!(err(s, "undefined"), EntryError::Undefined);
            assert_eq!(err(s, "n::S::g"), EntryError::HasParameters);
            for e in [err(s, "missing"), err(s, "ov"), err(s, "params")] {
                assert!(!e.to_string().is_empty());
            }
            for (name, v) in
                [("n::S::f", 5), ("::n::S::f", 5), ("T::f", 5), ("n::in::h", 7), ("alias::h", 7)]
            {
                assert_eq!(ok(&run(s, name)), RunValue::Signed(v), "{name}");
            }
            // An entry by declaration, as a client holding a DeclId does.
            let e = s.entry_named("n::in::h").unwrap();
            assert_eq!(s.entry(e.decl()), Ok(e));
        },
    );
}

#[test]
fn a_translation_unit_with_errors_is_refused() {
    with_session("int x = ;\nint y = ;\n", |s, _, sink| {
        assert_eq!(s.unwrap_err(), OpenError::TuHasErrors { errors: 2 });
        assert_eq!(sink.take().len(), 2);
    });
}

#[test]
fn a_session_moves_to_another_thread() {
    with_session(FIB, |s, _, _| {
        let mut s = s.expect("session opens");
        let r = std::thread::scope(|sc| {
            sc.spawn(move || {
                let r = run(&mut s, "small_fib");
                drop(s.into_sema());
                r
            })
            .join()
            .unwrap()
        });
        assert_eq!(ok(&r), RunValue::Signed(144));
    });
}
