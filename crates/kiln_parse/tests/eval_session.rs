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
    CancelToken, ConstexprLimits, Entity, EntryError, EvalSession, HostCalls, HostError, HostOut,
    OpenError, ParseOptions, RunBudget, RunError, RunReport, RunValue, ValueError,
    parse_preprocessed_retained,
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

/// The diagnostic options of every engine here.
fn dopts() -> DiagOptions {
    DiagOptions { clang_compat: ParseOptions::default().clang_compat, ..DiagOptions::default() }
}

/// `ds` rendered as the driver renders diagnostics (the public renderer,
/// `kiln_diag::render_diagnostics`, WP-2324), one string per line.
fn render(sm: &SourceManager, ds: &[Diagnostic]) -> Vec<String> {
    kiln_diag::render_diagnostics(sm, &dopts(), ds).lines().map(str::to_owned).collect()
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
    let engine = DiagEngine::new(&sm, &dopts(), sink.clone());
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
        Ok(v) => v.clone(),
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

/// `RunError::Uncaught` of type `ty` (spelled), as [`shape`] leaves it.
fn uncaught(ty: &str, what: Option<&str>, scalars: &[u64]) -> RunError {
    RunError::Uncaught {
        ty: Entity { display: ty.into(), key: ty.into(), decl: None },
        what: what.map(str::to_owned),
        scalars: scalars.to_vec(),
        value: Err(String::new()),
    }
}

/// `e` with an `Uncaught`'s declaration and value dropped (compared
/// where the tests below need them).
fn shape(e: &RunError) -> RunError {
    match e {
        RunError::Uncaught { ty, what, scalars, .. } => RunError::Uncaught {
            ty: Entity { display: Arc::clone(&ty.display), key: Arc::clone(&ty.key), decl: None },
            what: what.clone(),
            scalars: scalars.clone(),
            value: Err(String::new()),
        },
        e => e.clone(),
    }
}

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
            ("thrown", tiny, None, uncaught("E", Some("bad thing"), &[]), ""),
            ("leak", tiny, None, RunError::Failed, "not deallocated"),
            ("nothrow_exit", tiny, None, RunError::Failed, ""),
        ];
        for (name, budget, cancel, want, needle) in cases {
            for _ in 0..2 {
                let e = s.entry_named(name).unwrap();
                let token = CancelToken::new();
                let r = s.run(e, &mut Counter::default(), budget, cancel.unwrap_or(&token));
                let shown = render(sm, &r.diagnostics);
                assert_eq!(
                    r.result.as_ref().err().map(shape),
                    Some(want.clone()),
                    "{name}: {shown:?}"
                );
                // WP-2503: only the handler's own error is the host's.
                assert_eq!(r.result.as_ref().err().map(RunError::is_host), Some(name == "herr"));
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

/// WP-2480 (spec/eval-session.md §8, spec/host-calls.md §2): a host call
/// kiln refuses because a check of its arguments failed is
/// `RunError::HostArgument`, and its handler is not called; the handler's
/// own error stays `RunError::Host`. Only the second is the host's.
#[test]
fn an_argument_check_is_not_the_handlers_error() {
    session(
        "consteval u64 null_in() { return hc(2, nullptr, 1); }\n\
         consteval u64 short_in() { u64 a[2]{1, 2}; return hc(2, a, 3); }\n\
         consteval u64 uninit_in() { u64 a[2]; a[0] = 1; return hc(2, a, 2); }\n\
         consteval u64 const_out() { const u64 o[1]{}; return hc(2, nullptr, 0, nullptr, 0, \
           const_cast<u64*>(o), 1); }\n\
         consteval u64 herr() { return hc(3); }\n\
         consteval u64 good() { u64 a[1]{21}; return hc(2, a, 1); }\n",
        |s, sm| {
            let cases = [
                ("null_in", "'in' of __builtin_kiln_host_call is null with a length of 1"),
                (
                    "short_in",
                    "'in' of __builtin_kiln_host_call does not point to 3 consecutive elements",
                ),
                ("uninit_in", "'in' of __builtin_kiln_host_call element 1 is uninitialized"),
                ("const_out", "'out' of __builtin_kiln_host_call designates a const object"),
            ];
            for _ in 0..2 {
                for (name, message) in cases {
                    let mut h = Counter::default();
                    let e = s.entry_named(name).unwrap();
                    let r = s.run(e, &mut h, RunBudget::PROGRAM, &CancelToken::new());
                    let shown = render(sm, &r.diagnostics);
                    assert_eq!(
                        r.result,
                        Err(RunError::HostArgument { message: message.into() }),
                        "{name}: {shown:?}"
                    );
                    assert!(!r.result.as_ref().unwrap_err().is_host());
                    assert_eq!((h.seen, r.host_calls), (0, 0), "{name}: the handler ran");
                    assert!(shown.iter().any(|d| d.contains(message)), "{name}: {shown:?}");
                }
                let mut h = Counter::default();
                let e = s.entry_named("herr").unwrap();
                let r = s.run(e, &mut h, RunBudget::PROGRAM, &CancelToken::new());
                assert_eq!(r.result, Err(RunError::Host { message: "boom".into() }));
                assert!(r.result.as_ref().unwrap_err().is_host());
                assert_eq!(h.seen, 1);
                assert_eq!(ok(&run(s, "good")), RunValue::Unsigned(42));
            }
        },
    );
}

#[test]
fn failure_diagnostics_have_call_stack_notes_and_repeat() {
    session(FAILURES, |s, sm| {
        let first = render(sm, &run(s, "bad").diagnostics);
        assert!(first.iter().any(|d| d.contains("in call to 'at(5)'")), "{first:?}");
        assert!(first[0].starts_with("t.cpp:9:"), "{first:?}");
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
            assert!(
                shown.iter().any(|d| d.contains("a host call cannot be evaluated here")),
                "{shown:?}"
            );
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
         int not_constexpr() { return 3; }\n\
         consteval int params(int x) { return x; }\n\
         inline constexpr int k = 1;\n\
         consteval const int& real() { return k; }\n\
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
            assert_eq!(err(s, "not_constexpr"), EntryError::NotConstexpr);
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

/// WP-2410: templates only a run instantiates (`E<int>`'s member
/// functions are defined at their first call, in the run), shallow and
/// deep: `R<T, N>` instantiates `R<T, N - 1>` from its initializer, one
/// native recursion of the instantiator per level.
const STACK_SRC: &str = "template <class T, int N> struct R { static constexpr int v = R<T, N - 1>::v + 1; };\n\
template <class T> struct R<T, 0> { static constexpr int v = 0; };\n\
template <class T> struct E {\n\
  static consteval int shallow() { return R<T, 8>::v; }\n\
  static consteval int shallow2() { return R<T, 16>::v; }\n\
  static consteval int deep() { return R<T, 1000>::v; }\n\
};\n\
static_assert(sizeof(E<int>) > 0);\n\
using X = E<int>;\n";

/// `f` on a new thread with a `size`-byte stack and the stack guard
/// installed on it, as the driver's compile thread has (and a client's
/// worker may).
fn on_guarded_thread<'s, R: Send + 's>(
    sc: &'s std::thread::Scope<'s, '_>,
    size: usize,
    f: impl FnOnce() -> R + Send + 's,
) -> R {
    std::thread::Builder::new()
        .stack_size(size)
        .spawn_scoped(sc, move || {
            kiln_sema::stack_guard::StackGuard::install(size);
            f()
        })
        .expect("spawn")
        .join()
        .expect("join")
}

/// WP-2410 (spec/eval-session.md §9): a run checks the stack of the thread
/// it runs on, not the one the session was compiled on. Compiled on
/// thread A (guarded), a run on thread B that instantiates a template
/// succeeds, then a run on A; a run on B that exhausts B's stack fails
/// with the stack diagnostic, and the session stays usable on A and B.
#[test]
fn a_run_checks_the_stack_of_its_own_thread() {
    const BIG: usize = 256 << 20;
    const SMALL: usize = 4 << 20;
    std::thread::scope(|outer| {
        on_guarded_thread(outer, BIG, || {
            session(STACK_SRC, |s, sm| {
                let on = |size: usize, s: &mut EvalSession<'_, '_>, name: &str| {
                    std::thread::scope(|sc| on_guarded_thread(sc, size, || run(s, name)))
                };
                // Compiled on A, run on B: instantiates `R<int, 1..8>`.
                let b = on(BIG, s, "X::shallow");
                assert_eq!(ok(&b), RunValue::Signed(8), "{:?}", render(sm, &b.diagnostics));
                // Then on A, more instantiation.
                let a = run(s, "X::shallow2");
                assert_eq!(ok(&a), RunValue::Signed(16), "{:?}", render(sm, &a.diagnostics));
                // A run that exhausts B's stack (4 MiB) fails with the
                // stack diagnostic, as a run on the driver's thread would.
                // Each level of `R` both instantiates and evaluates, so
                // which of the two finds the stack low depends on where
                // in a level the red zone falls (a few bytes of frame
                // move it): the phase is either.
                let deep = on(SMALL, s, "X::deep");
                assert_eq!(deep.result, Err(RunError::StackExhausted));
                let text = render(sm, &deep.diagnostics).join("\n");
                let headline = text
                    .lines()
                    .find(|l| l.contains("fatal error: kiln: stack exhausted during "))
                    .unwrap_or_else(|| panic!("{text}"))
                    .to_owned();
                assert!(
                    ["template instantiation", "constant evaluation"]
                        .iter()
                        .any(|p| headline.contains(&format!("stack exhausted during {p};"))),
                    "{text}"
                );
                // The exhaustion was the run's: later runs on either
                // thread are not stopped by it.
                let a = run(s, "X::shallow");
                assert_eq!(ok(&a), RunValue::Signed(8), "{:?}", render(sm, &a.diagnostics));
                let b = on(SMALL, s, "X::shallow2");
                assert_eq!(ok(&b), RunValue::Signed(16), "{:?}", render(sm, &b.diagnostics));
                // The instantiation the exhaustion stopped is kept as it
                // failed (semantic work is not rolled back, §5.1 rule 1):
                // every run reaching it fails alike, on any thread.
                let again = run(s, "X::deep");
                assert_eq!(again.result, Err(RunError::StackExhausted));
                let again = render(sm, &again.diagnostics).join("\n");
                assert!(again.contains(&headline), "{again}");
            });
            // A fresh session on A's stack runs the deep entry.
            session(STACK_SRC, |s, sm| {
                let r = run(s, "X::deep");
                assert_eq!(ok(&r), RunValue::Signed(1000), "{:?}", render(sm, &r.diagnostics));
            });
        });
    });
}

/// WP-2420: static data members only a read instantiates (each level's
/// initializer instantiates the next), shallow and deep, and an entry that
/// reaches the deep one.
const VALUE_STACK_SRC: &str = "template <class T, int N> struct R { static constexpr int v = R<T, N - 1>::v + 1; };\n\
template <class T> struct R<T, 0> { static constexpr int v = 0; };\n\
template <class T> struct E { static consteval int deep() { return R<T, 1000>::v; } };\n\
static_assert(sizeof(R<int, 8>) + sizeof(R<int, 16>) + sizeof(R<char, 8>) + sizeof(R<char, 16>) > 0);\n\
static_assert(sizeof(R<int, 1000>) + sizeof(E<int>) > 0);\n\
using I8 = R<int, 8>; using I16 = R<int, 16>; using C8 = R<char, 8>; using C16 = R<char, 16>;\n\
using D = R<int, 1000>; using X = E<int>;\n";

/// WP-2420 (spec/eval-session.md §9): `variable_value` checks the stack of
/// the thread it is called on, as a run does. Compiled on thread A, a read
/// on thread B that instantiates succeeds, then one on A; a read on a
/// small-stack thread that exhausts it is refused with
/// `ValueError::StackExhausted` (WP-2510; the exhaustion is diagnosed: a
/// run reaching the same initializer reports it, and so does a later read
/// of it), and later reads on either thread are not stopped by it.
#[test]
fn a_variable_value_checks_the_stack_of_its_own_thread() {
    const BIG: usize = 256 << 20;
    const SMALL: usize = 4 << 20;
    const CAP: u64 = RunBudget::PROGRAM.result_bytes;
    let read =
        |s: &mut EvalSession<'_, '_>, name: &str| s.variable_value_named(name, CAP).map(|t| t.root);
    std::thread::scope(|outer| {
        on_guarded_thread(outer, BIG, || {
            session(VALUE_STACK_SRC, |s, sm| {
                let on = |size: usize, s: &mut EvalSession<'_, '_>, name: &str| {
                    std::thread::scope(|sc| on_guarded_thread(sc, size, || read(s, name)))
                };
                // Compiled on A, read on B: instantiates `R<int, 1..8>`.
                assert_eq!(on(BIG, s, "I8::v"), Ok(RunValue::Signed(8)));
                // Then on A, more instantiation.
                assert_eq!(read(s, "I16::v"), Ok(RunValue::Signed(16)));
                // A read that exhausts the small thread's stack is refused
                // as a stack exhaustion (WP-2510).
                assert_eq!(on(SMALL, s, "D::v"), Err(ValueError::StackExhausted));
                // The exhaustion was the read's: later reads on either
                // thread are not stopped by it.
                assert_eq!(read(s, "C8::v"), Ok(RunValue::Signed(8)));
                assert_eq!(on(SMALL, s, "C16::v"), Ok(RunValue::Signed(16)));
                // The instantiation it stopped stays failed (§9), and a run
                // reaching it reports the exhaustion.
                assert_eq!(read(s, "D::v"), Err(ValueError::StackExhausted));
                let r = run(s, "X::deep");
                assert_eq!(r.result, Err(RunError::StackExhausted));
                let text = render(sm, &r.diagnostics).join("\n");
                // The phase is not pinned: it depends on frame sizes
                // (spec/sema-templates.md §10.3).
                assert!(
                    ["template instantiation", "constant evaluation"].iter().any(|p| text
                        .contains(&format!("fatal error: kiln: stack exhausted during {p};"))),
                    "{text}"
                );
            });
            // A fresh session on A's stack reads the deep one.
            session(VALUE_STACK_SRC, |s, _| {
                assert_eq!(read(s, "D::v"), Ok(RunValue::Signed(1000)));
                assert_eq!(ok(&run(s, "X::deep")), RunValue::Signed(1000));
            });
        });
    });
}

/// WP-2321 (spec/host-calls.md §3, spec/eval-session.md §6): a run's
/// handler is borrowed for the call, so it need not be `Send`: this one
/// shares its count with the test through an `Rc<Cell<u32>>`, and borrows
/// a value of the caller's frame.
#[test]
fn a_runs_handler_need_not_be_send() {
    struct Shared<'b> {
        count: std::rc::Rc<std::cell::Cell<u32>>,
        base: &'b u64,
    }
    impl HostCalls for Shared<'_> {
        fn call(
            &mut self,
            op: u32,
            _ints: &[u64],
            _bytes: &[u8],
            _out: &mut HostOut<'_>,
        ) -> Result<u64, HostError> {
            self.count.set(self.count.get() + 1);
            Ok(*self.base + u64::from(op))
        }
    }
    session("consteval u64 twice() { return hc(1) + hc(2); }\n", |s, _| {
        let count = std::rc::Rc::new(std::cell::Cell::new(0));
        let base = 100;
        let e = s.entry_named("twice").unwrap();
        for n in 1..=3 {
            let mut h = Shared { count: std::rc::Rc::clone(&count), base: &base };
            let r = s.run(e, &mut h, RunBudget::PROGRAM, &CancelToken::new());
            assert_eq!(ok(&r), RunValue::Unsigned(203));
            assert_eq!(r.host_calls, 2);
            assert_eq!(count.get(), 2 * n);
        }
        // The session kept no handle on the handler.
        assert_eq!(std::rc::Rc::strong_count(&count), 1);
    });
}

/// WP-2324 (spec/eval-session.md §8): `RunError::Uncaught` carries the
/// thrown object's first four integral or enumeration data members in
/// declaration order, a base's first, for a class that derives from
/// `std::exception` and for one that does not; signed values in two's
/// complement; bit-fields by their value, unnamed ones skipped; members
/// of other types skipped; an uninitialized member ends the list.
#[test]
fn uncaught_exceptions_carry_their_leading_scalars() {
    session(
        "namespace std { struct exception { \
         constexpr virtual ~exception() {} \
         constexpr virtual const char* what() const noexcept { return \"e\"; } }; }\n\
         enum class Code : short { a = 3, b = -2 };\n\
         struct Err : std::exception { int line; double d; Code code; \
           constexpr Err(int l, Code c) : line(l), d(1.5), code(c) {} \
           constexpr const char* what() const noexcept override { return \"err\"; } };\n\
         struct Base { unsigned char tag = 9; };\n\
         struct Plain : Base { const char* s = \"x\"; long long big = -5; bool ok = true; \
           int : 3; int bits : 4 = -3; int fifth = 77; };\n\
         struct Empty {};\n\
         struct Gap { int a = 1; int b; int c = 3; constexpr Gap() {} };\n\
         consteval int std_derived() { throw Err(12, Code::b); }\n\
         consteval int plain() { throw Plain{}; }\n\
         consteval int empty() { throw Empty{}; }\n\
         consteval int scalar() { throw -1; }\n\
         consteval int gap() { throw Gap(); }\n",
        |s, _| {
            let uncaught = |s: &mut EvalSession<'_, '_>, name: &str| match run(s, name).result {
                Err(RunError::Uncaught { ty, what, scalars, .. }) => {
                    (ty.display.to_string(), what, scalars)
                }
                r => panic!("{name}: {r:?}"),
            };
            assert_eq!(
                uncaught(s, "std_derived"),
                ("Err".into(), Some("err".into()), vec![12, (-2i64) as u64])
            );
            assert_eq!(
                uncaught(s, "plain"),
                ("Plain".into(), None, vec![9, (-5i64) as u64, 1, (-3i64) as u64])
            );
            assert_eq!(uncaught(s, "empty"), ("Empty".into(), None, Vec::new()));
            assert_eq!(uncaught(s, "scalar"), ("int".into(), None, Vec::new()));
            assert_eq!(uncaught(s, "gap"), ("Gap".into(), None, vec![1]));
            // Every run reads them alike.
            assert_eq!(uncaught(s, "plain").2, vec![9, (-5i64) as u64, 1, (-3i64) as u64]);
        },
    );
}
