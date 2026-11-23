//! WP-2073: nested work in evaluation sessions (spec/eval-session.md
//! §5.1): semantic work a run triggers is charged to the run, and a
//! run's report (its value, steps, nested steps, budget decision and
//! diagnostics) is the same on run 1 and run N, on a fresh and a used
//! session, and whatever other entries ran before it.
#![forbid(unsafe_code)]

use std::sync::Arc;

use kiln_diag::{DiagEngine, DiagOptions, Diagnostic, MemorySink};
use kiln_parse::{
    CancelToken, ConstexprLimits, EvalSession, HostCalls, HostError, HostOut, ParseOptions,
    RunBudget, RunError, RunReport, RunValue, parse_preprocessed_retained,
};
use kiln_pp::{ClangFeatures, ModuleOptions, PpOptions, Preprocessor};
use kiln_sema::target::{Arch, Target};
use kiln_source::SourceManager;
use kiln_source::symbol::Interner;

/// The wrapper every test source starts with.
const PRE: &str = "using u64 = unsigned long long;\n\
constexpr u64 hc(unsigned op) {\n\
return __builtin_kiln_host_call(op, nullptr, 0, nullptr, 0, nullptr, 0, nullptr, 0);\n\
}\n\
[[deprecated(\"use new\")]] constexpr int old(int x) { return x; }\n";

/// A run's handler: op 1 returns 7; anything else fails.
struct Seven;

impl HostCalls for Seven {
    fn call(
        &mut self,
        op: u32,
        _ints: &[u64],
        _bytes: &[u8],
        _out: &mut HostOut<'_>,
    ) -> Result<u64, HostError> {
        match op {
            1 => Ok(7),
            _ => Err(HostError::new(format!("unknown op {op}"))),
        }
    }
}

/// `ds` rendered as the driver renders diagnostics (the public renderer,
/// `kiln_diag::render_diagnostics`, WP-2324), one string per line.
fn render(sm: &SourceManager, ds: &[Diagnostic]) -> Vec<String> {
    let dopts = DiagOptions {
        clang_compat: ParseOptions::default().clang_compat,
        ..DiagOptions::default()
    };
    kiln_diag::render_diagnostics(sm, &dopts, ds).lines().map(str::to_owned).collect()
}

/// Parses `PRE` + `src` with the translation unit's step limit
/// `tu_steps`, opens a session and calls `f` with it.
fn session_with<R>(
    src: &str,
    tu_steps: Option<u64>,
    f: impl for<'a, 'sm> FnOnce(&mut EvalSession<'a, 'sm>, &'sm SourceManager) -> R,
) -> R {
    session_opts(src, tu_steps, false, f)
}

/// [`session_with`], with `-freflection` when `reflection`.
fn session_opts<R>(
    src: &str,
    tu_steps: Option<u64>,
    reflection: bool,
    f: impl for<'a, 'sm> FnOnce(&mut EvalSession<'a, 'sm>, &'sm SourceManager) -> R,
) -> R {
    let sm = SourceManager::new();
    let file = sm.add_virtual("t.cpp", format!("{PRE}{src}")).expect("location space");
    let interner = Arc::new(Interner::new());
    let sink = MemorySink::new();
    let opts = ParseOptions { reflection, ..ParseOptions::default() };
    let dopts = DiagOptions { clang_compat: opts.clang_compat, ..DiagOptions::default() };
    let engine = DiagEngine::new(&sm, &dopts, sink.clone());
    let mut ppo = PpOptions::new(opts.std);
    ppo.parser_pragmas = true;
    ppo.reflection = reflection;
    ppo.modules = Some(ModuleOptions::default());
    let mut pp = Preprocessor::new(&sm, &engine, &interner, ppo);
    let features = ClangFeatures::new("x86_64-unknown-linux-gnu", opts.std, &[])
        .expect("x86-64 features")
        .with_kiln_host_calls(true);
    pp.set_feature_queries(Box::new(features));
    pp.enter_main_file(file);
    let mut limits = ConstexprLimits::default();
    if let Some(n) = tu_steps {
        limits.budget.steps = n;
    }
    let (sema, _pp) = parse_preprocessed_retained(
        pp,
        Arc::clone(&interner),
        &engine,
        Target::for_arch(Arch::X86_64),
        &opts,
        limits,
        Some(Box::new(Seven)),
    );
    let tu = render(&sm, &sink.take());
    assert!(tu.is_empty(), "translation unit diagnostics:\n{}", tu.join("\n"));
    let mut s = EvalSession::open(sema).expect("session opens");
    let r = f(&mut s, &sm);
    let late = render(&sm, &sink.take());
    assert!(late.is_empty(), "runs' diagnostics reached the sink:\n{}", late.join("\n"));
    r
}

fn session<R>(
    src: &str,
    f: impl for<'a, 'sm> FnOnce(&mut EvalSession<'a, 'sm>, &'sm SourceManager) -> R,
) -> R {
    session_with(src, None, f)
}

/// What a run reports, as compared between runs.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Seen {
    result: Result<RunValue, RunError>,
    steps: u64,
    nested_steps: u64,
    calls: u64,
    nested_calls: u64,
    diags: Vec<String>,
}

fn seen(sm: &SourceManager, r: &RunReport) -> Seen {
    Seen {
        result: r.result.clone(),
        steps: r.steps,
        nested_steps: r.nested_steps,
        calls: r.calls,
        nested_calls: r.nested_calls,
        diags: render(sm, &r.diagnostics),
    }
}

fn run_c(
    s: &mut EvalSession<'_, '_>,
    sm: &SourceManager,
    name: &str,
    budget: RunBudget,
    cancel: &CancelToken,
) -> Seen {
    let e = s.entry_named(name).unwrap_or_else(|e| panic!("entry {name}: {e}"));
    let r = s.run(e, &mut Seven, budget, cancel);
    seen(sm, &r)
}

fn run_b(s: &mut EvalSession<'_, '_>, sm: &SourceManager, name: &str, budget: RunBudget) -> Seen {
    run_c(s, sm, name, budget, &CancelToken::new())
}

fn run(s: &mut EvalSession<'_, '_>, sm: &SourceManager, name: &str) -> Seen {
    run_b(s, sm, name, RunBudget::PROGRAM)
}

/// Each entry of `names` run first in a session of its own.
fn fresh(src: &str, names: &[&str]) -> Vec<Seen> {
    names.iter().map(|n| session(src, |s, sm| run(s, sm, n))).collect()
}

/// Entry `name` run under `budget` first in a session of its own (the
/// translation unit's step limit `tu_steps`).
fn fresh_b(src: &str, tu_steps: Option<u64>, name: &str, budget: RunBudget) -> Seen {
    session_with(src, tu_steps, |s, sm| run_b(s, sm, name, budget))
}

fn has(s: &Seen, text: &str) -> bool {
    s.diags.iter().any(|d| d.contains(text))
}

// ---- the existing holes (found while designing WP-2073) -------------------

/// A call memoized after a recorded function was reached in a run must not
/// let a later run skip that function's replay.
const MEMO_SRC: &str = "template <class T> struct M {\n\
  static constexpr int noisy(int n) { return old(T{}) + n; }\n\
  static constexpr int g(int n) { return noisy(n) + 1; }\n\
  static consteval int a() { noisy(0); return g(1); }\n\
  static consteval int b() { return g(1); }\n\
};\n\
static_assert(sizeof(M<int>) > 0);\n\
using X = M<int>;\n";

#[test]
fn a_memoized_call_does_not_skip_a_replay() {
    let f = fresh(MEMO_SRC, &["X::b"]);
    assert!(has(&f[0], "'old' is deprecated"), "{:?}", f[0]);
    session(MEMO_SRC, |s, sm| {
        let a = run(s, sm, "X::a");
        assert!(has(&a, "'old' is deprecated"), "{a:?}");
        let b = run(s, sm, "X::b");
        assert_eq!(b, f[0]);
    });
}

/// Work shared by two units (a class both instantiate) is replayed by the
/// second one too.
const SHARED_SRC: &str = "template <class T> struct C { static constexpr int v = old(T{}); };\n\
template <class T> struct A { static consteval int f() { return C<T>::v; } };\n\
template <class T> struct B { static consteval int f() { return C<T>::v + 1; } };\n\
static_assert(sizeof(A<int>) + sizeof(B<int>) > 0);\n\
using XA = A<int>;\n\
using XB = B<int>;\n";

#[test]
fn shared_work_is_replayed_by_every_unit_that_needs_it() {
    let f = fresh(SHARED_SRC, &["XA::f", "XB::f"]);
    assert!(has(&f[0], "'old' is deprecated"), "{:?}", f[0]);
    assert!(has(&f[1], "'old' is deprecated"), "{:?}", f[1]);
    session(SHARED_SRC, |s, sm| {
        assert_eq!(run(s, sm, "XA::f"), f[0]);
        assert_eq!(run(s, sm, "XB::f"), f[1]);
    });
    session(SHARED_SRC, |s, sm| {
        assert_eq!(run(s, sm, "XB::f"), f[1]);
        assert_eq!(run(s, sm, "XA::f"), f[0]);
    });
}

/// An exception specification resolved in one run is replayed in the next
/// (not reproduced: sema resolves the specification at the use, inside the
/// unit that compiles it; these pin the behaviour).
const NOTHROW_SRCS: [&str; 3] = [
    "template <class T> constexpr void thrower(T t) noexcept(old(T{}) == 1) { throw 1; }\n\
template <class T> struct H {\n\
  static consteval int e() { try { thrower(T{}); } catch (int) { return 1; } return 0; }\n\
};\n\
static_assert(sizeof(H<int>) > 0);\n\
using X = H<int>;\n",
    "template <class T> struct Q { constexpr void th() noexcept(old(T{}) == 1) { throw 1; } };\n\
template <class T> struct H {\n\
  static consteval int e() { Q<T> q; auto p = &Q<T>::th; try { (q.*p)(); } catch (int) { return 1; } return 0; }\n\
};\n\
static_assert(sizeof(H<int>) > 0);\n\
using X = H<int>;\n",
    "template <class T> struct Q { virtual constexpr void th() noexcept(old(T{}) == 1) { throw 1; } };\n\
template <class T> struct H {\n\
  static consteval int e() { Q<T> q; Q<T>& r = q; try { r.th(); } catch (int) { return 1; } return 0; }\n\
};\n\
static_assert(sizeof(H<int>) > 0);\n\
using X = H<int>;\n",
];

#[test]
fn a_resolved_exception_specification_is_replayed() {
    for src in NOTHROW_SRCS {
        let f = fresh(src, &["X::e"]);
        session(src, |s, sm| {
            for _ in 0..3 {
                assert_eq!(run(s, sm, "X::e"), f[0]);
            }
        });
    }
}

/// An initializer the Oven evaluates for a read: a static data member's
/// out-of-line one, a variable template's, an in-class one, a namespace
/// variable's (evaluated with the translation unit).
const VAR_SRCS: [&str; 4] = [
    "constexpr int big(int n) { int s = 0; for (int i = 0; i < n; ++i) s += i; return s; }\n\
template <class T> struct S { static const int v; };\n\
template <class T> const int S<T>::v = big(sizeof(T) * 500);\n\
template <class T> struct U { static consteval int e() { return S<T>::v; } };\n\
static_assert(sizeof(U<int>) > 0);\nusing X = U<int>;\n",
    "constexpr int big(int n) { int s = 0; for (int i = 0; i < n; ++i) s += i; return s; }\n\
template <class T> const int vt = big(sizeof(T) * 500);\n\
template <class T> struct U { static consteval int e() { return vt<T>; } };\n\
static_assert(sizeof(U<int>) > 0);\nusing X = U<int>;\n",
    "constexpr int big(int n) { int s = 0; for (int i = 0; i < n; ++i) s += i; return s; }\n\
template <class T> struct S { static const int v = big(sizeof(T) * 500); };\n\
template <class T> struct U { static consteval int e() { return S<T>::v; } };\n\
static_assert(sizeof(U<int>) > 0);\nusing X = U<int>;\n",
    "constexpr int big(int n) { int s = 0; for (int i = 0; i < n; ++i) s += i; return s; }\n\
const int k = big(2000);\n\
consteval int e() { return k; }\nstruct X { static consteval int e() { return ::e(); } };\n",
];

/// A run whose budget its nested work exceeds keeps no failure: the
/// initializer runs under the translation unit's limits, the run fails at
/// its budget, and every later run is as in a fresh session.
#[test]
fn an_initializer_read_under_a_small_budget_keeps_no_failure() {
    let small = RunBudget::PROGRAM.with_steps(50);
    for src in VAR_SRCS {
        let f = fresh(src, &["X::e"]);
        assert!(f[0].result.is_ok(), "{:?}", f[0]);
        let fs = fresh_b(src, None, "X::e", small);
        session(src, |s, sm| {
            assert_eq!(run_b(s, sm, "X::e", small), fs);
            assert_eq!(run(s, sm, "X::e"), f[0]);
            assert_eq!(run_b(s, sm, "X::e", small), fs);
            assert_eq!(run(s, sm, "X::e"), f[0]);
        });
    }
}

/// Found by WP-2072: a member function instantiated by another
/// function's instantiation (`c`'s body uses `a`) was compiled later in
/// the same run without reaching its instantiation, so the next run that
/// called it first (`X::a`) was charged nothing and replayed nothing.
const INST_BY_OTHER_SRC: &str = "constexpr int fib(int n) { return n < 2 ? n : fib(n - 1) + fib(n - 2); }\n\
template <class T> struct W {\n\
  static constexpr int k = sizeof(T) + fib(10);\n\
  static constexpr int helper() { return old(k + T{}) + fib(12); }\n\
  static consteval int a() { return helper() + 1; }\n\
  static consteval int c() { return a() + 2; }\n\
};\n\
static_assert(sizeof(W<long>) > 0);\n\
using X = W<long>;\n";

#[test]
fn a_function_instantiated_by_other_work_is_reached_by_its_compile() {
    let f = fresh(INST_BY_OTHER_SRC, &["X::a", "X::c"]);
    assert!(has(&f[0], "'old' is deprecated") && f[0].nested_steps > 0, "{:?}", f[0]);
    session(INST_BY_OTHER_SRC, |s, sm| {
        for _ in 0..2 {
            assert_eq!(run(s, sm, "X::c"), f[1]);
            assert_eq!(run(s, sm, "X::a"), f[0]);
        }
    });
}

// ---- the acceptance criterion: determinism ---------------------------------

/// Nested work inside nested work (`Outer<T>::v`'s evaluation calls
/// `get<T>`, whose instantiation reads `Inner<T>::v`), shared by two
/// entries, with memoized calls and a diagnostic.
const NEST_SRC: &str = "constexpr int big(int n) { int s = 0; for (int i = 0; i < n; ++i) s += i; return s; }\n\
template <class T> struct Inner { static constexpr int v = big(300) + old(T{}); };\n\
template <class T> constexpr int get() { return Inner<T>::v + 1; }\n\
template <class T> struct Outer { static constexpr int v = get<T>() + big(200); };\n\
template <class T> struct A { static consteval int f() { return Outer<T>::v + big(50); } };\n\
template <class T> struct B { static consteval int f() { return Inner<T>::v + big(50) + big(50); } };\n\
static_assert(sizeof(A<int>) + sizeof(B<int>) > 0);\n\
using XA = A<int>;\n\
using XB = B<int>;\n";

#[test]
fn reports_do_not_depend_on_what_ran_before() {
    let f = fresh(NEST_SRC, &["XA::f", "XB::f"]);
    for x in &f {
        assert!(x.result.is_ok(), "{x:?}");
        assert!(x.nested_steps > 0 && x.nested_calls > 0, "{x:?}");
        assert!(has(x, "'old' is deprecated"), "{x:?}");
    }
    assert!(f[0].nested_steps > f[1].nested_steps, "{f:?}");
    for order in [["XA::f", "XB::f"], ["XB::f", "XA::f"]] {
        session(NEST_SRC, |s, sm| {
            for _ in 0..3 {
                for n in order {
                    let want = &f[usize::from(n == "XB::f")];
                    assert_eq!(&run(s, sm, n), want, "{n} in order {order:?}");
                }
            }
        });
    }
}

/// The budget decision is made on the run's own steps plus its charged
/// nested steps, alike for a run that does the nested work and one that
/// replays it.
#[test]
fn the_budget_counts_charged_nested_steps() {
    let f = fresh(NEST_SRC, &["XA::f", "XB::f"]);
    for (i, n) in ["XA::f", "XB::f"].into_iter().enumerate() {
        let total = f[i].steps + f[i].nested_steps;
        let exact = RunBudget::PROGRAM.with_steps(total);
        let short = RunBudget::PROGRAM.with_steps(total - 1);
        let nested_only = RunBudget::PROGRAM.with_steps(f[i].nested_steps - 1);
        assert_eq!(fresh_b(NEST_SRC, None, n, exact), f[i]);
        let fs = fresh_b(NEST_SRC, None, n, short);
        assert_eq!(fs.result, Err(RunError::StepBudget), "{fs:?}");
        let fn_ = fresh_b(NEST_SRC, None, n, nested_only);
        assert_eq!(fn_.result, Err(RunError::StepBudget), "{fn_:?}");
        session(NEST_SRC, |s, sm| {
            // A sequence of runs whose budget the nested work exceeds.
            assert_eq!(run_b(s, sm, n, nested_only), fn_);
            assert_eq!(run_b(s, sm, n, nested_only), fn_);
            assert_eq!(run_b(s, sm, n, short), fs);
            assert_eq!(run_b(s, sm, n, exact), f[i]);
            assert_eq!(run_b(s, sm, n, short), fs);
            assert_eq!(run(s, sm, n), f[i]);
        });
        // Used by the other entry first.
        session(NEST_SRC, |s, sm| {
            let _ = run(s, sm, if i == 0 { "XB::f" } else { "XA::f" });
            assert_eq!(run_b(s, sm, n, short), fs);
            assert_eq!(run_b(s, sm, n, exact), f[i]);
        });
    }
}

/// Depth and bytes: nested work's depth and memory count on top of the
/// run's, alike fresh and used.
const DEEP_SRC: &str = "constexpr int rec(int n) { return n == 0 ? 0 : 1 + rec(n - 1); }\n\
constexpr int heap(int n) { int* p = new int[n]; for (int i = 0; i < n; ++i) p[i] = i; int r = p[n - 1]; delete[] p; return r; }\n\
template <class T> struct D { static constexpr int v = rec(sizeof(T) * 25); };\n\
template <class T> struct H { static constexpr int v = heap(sizeof(T) * 25000); };\n\
template <class T> struct E { static consteval int d() { return D<T>::v; } static consteval int h() { return H<T>::v; } };\n\
static_assert(sizeof(E<int>) > 0);\n\
using X = E<int>;\n";

#[test]
fn nested_depth_and_bytes_count_against_the_budget() {
    let cases = [
        ("X::d", RunBudget::PROGRAM.with_depth(50), RunError::DepthLimit),
        ("X::h", RunBudget::PROGRAM.with_bytes(1 << 16), RunError::MemoryBudget),
    ];
    for (n, tight, kind) in cases {
        let full = fresh(DEEP_SRC, &[n]).remove(0);
        assert!(full.result.is_ok(), "{full:?}");
        let ft = fresh_b(DEEP_SRC, None, n, tight);
        assert_eq!(ft.result, Err(kind), "{ft:?}");
        session(DEEP_SRC, |s, sm| {
            assert_eq!(run(s, sm, n), full);
            assert_eq!(run_b(s, sm, n, tight), ft);
            assert_eq!(run(s, sm, n), full);
        });
        session(DEEP_SRC, |s, sm| {
            assert_eq!(run_b(s, sm, n, tight), ft);
            assert_eq!(run_b(s, sm, n, tight), ft);
            assert_eq!(run(s, sm, n), full);
        });
    }
}

/// A run cancelled before its first step: the nested work it started runs
/// to completion and is kept, and the next run is as in a fresh session.
#[test]
fn cancellation_leaves_nested_work_as_fresh() {
    let f = fresh(NEST_SRC, &["XA::f"]).remove(0);
    session(NEST_SRC, |s, sm| {
        let cancel = CancelToken::new();
        cancel.cancel();
        let c = run_c(s, sm, "XA::f", RunBudget::PROGRAM, &cancel);
        assert_eq!(c.result, Err(RunError::Cancelled), "{c:?}");
        let c2 = run_c(s, sm, "XA::f", RunBudget::PROGRAM, &cancel);
        assert_eq!(c2.result, Err(RunError::Cancelled), "{c2:?}");
        assert_eq!(run(s, sm, "XA::f"), f);
        assert_eq!(run(s, sm, "XA::f"), f);
    });
}

/// A host call in nested work is refused the same way for every run that
/// reaches it.
const HOST_SRC: &str = "template <class T> struct H { static constexpr u64 v = hc(1); };\n\
template <class T> struct P { static consteval u64 f() { return H<T>::v; } };\n\
template <class T> struct Q { static consteval u64 f() { return H<T>::v + 1; } static consteval u64 g() { return hc(1); } };\n\
static_assert(sizeof(P<int>) + sizeof(Q<int>) > 0);\n\
using XP = P<int>;\n\
using XQ = Q<int>;\n";

#[test]
fn a_host_call_in_nested_work_fails_every_run_alike() {
    let f = fresh(HOST_SRC, &["XP::f", "XQ::f"]);
    for x in &f {
        assert_eq!(x.result, Err(RunError::Failed), "{x:?}");
        assert!(has(x, NESTED_HOST_NOTE), "{x:?}");
    }
    for order in [["XP::f", "XQ::f"], ["XQ::f", "XP::f"]] {
        session(HOST_SRC, |s, sm| {
            for _ in 0..2 {
                for n in order {
                    let want = &f[usize::from(n == "XQ::f")];
                    assert_eq!(&run(s, sm, n), want, "{n} in order {order:?}");
                    // The run's own host calls still reach its handler.
                    assert_eq!(run(s, sm, "XQ::g").result, Ok(RunValue::Unsigned(7)));
                }
            }
        });
    }
}

/// The note of a host call refused in nested work (spec/eval-session.md §6).
const NESTED_HOST_NOTE: &str = "a host call cannot be evaluated here: this expression is \
evaluated when the code is instantiated, not when it runs";

/// WP-2325 (spec/eval-session.md §6): the instantiation that makes a host
/// call is first reached in run 1; every one of N runs fails with the
/// same `RunError` and the same diagnostics, rendered by the public
/// renderer as the driver prints them: the refusal's note in the author's
/// terms, with the instantiation's context notes.
#[test]
fn every_run_reaching_a_refused_host_call_fails_identically() {
    session(HOST_SRC, |s, sm| {
        let e = s.entry_named("XP::f").unwrap();
        let first = s.run(e, &mut Seven, RunBudget::PROGRAM, &CancelToken::new());
        assert_eq!(first.result, Err(RunError::Failed));
        let shown = render(sm, &first.diagnostics);
        let text = shown.join("\n");
        assert!(text.contains(&format!("note: {NESTED_HOST_NOTE}")), "{text}");
        assert!(
            text.contains("note: in instantiation of static data member 'H<int>::v'"),
            "{text}"
        );
        for n in 2..=5 {
            let r = s.run(e, &mut Seven, RunBudget::PROGRAM, &CancelToken::new());
            assert_eq!(r.result, first.result, "run {n}");
            assert_eq!(render(sm, &r.diagnostics), shown, "run {n}");
        }
    });
}

/// Nested work that hits the translation unit's own step limit fails the
/// same way for every run that reaches it, fresh or used.
const LIMIT_SRC: &str = "constexpr int big(int n) { int s = 0; for (int i = 0; i < n; ++i) s += i; return s; }\n\
template <class T> struct L { static constexpr int v = big(sizeof(T) * 25000); };\n\
template <class T> struct LA { static consteval int f() { return L<T>::v; } };\n\
template <class T> struct LB { static consteval int f() { return L<T>::v + 1; } };\n\
static_assert(sizeof(LA<int>) + sizeof(LB<int>) > 0);\n\
using XA = LA<int>;\n\
using XB = LB<int>;\n";

#[test]
fn nested_work_over_the_translation_units_limit_fails_alike() {
    let tu = Some(5000);
    let f: Vec<Seen> =
        ["XA::f", "XB::f"].iter().map(|n| fresh_b(LIMIT_SRC, tu, n, RunBudget::PROGRAM)).collect();
    for x in &f {
        assert_eq!(x.result, Err(RunError::Failed), "{x:?}");
        assert!(has(x, "maximum step limit"), "{x:?}");
    }
    for order in [["XA::f", "XB::f"], ["XB::f", "XA::f"]] {
        session_with(LIMIT_SRC, tu, |s, sm| {
            for _ in 0..2 {
                for n in order {
                    let want = &f[usize::from(n == "XB::f")];
                    assert_eq!(&run(s, sm, n), want, "{n} in order {order:?}");
                }
            }
        });
    }
}

// ---- WP-2075: the units of class instantiation, exception specifications,
// constraints and default member initializers --------------------------------

/// `src` run with entries `a` and `b`, each in a fresh session as the
/// reference, then in used sessions in both orders, three rounds each.
fn shared_by_two(src: &str, a: &str, b: &str, tu_steps: Option<u64>) -> Vec<Seen> {
    shared_by_two_opts(src, a, b, tu_steps, false)
}

/// [`shared_by_two`], with `-freflection` when `reflection`.
fn shared_by_two_opts(
    src: &str,
    a: &str,
    b: &str,
    tu_steps: Option<u64>,
    reflection: bool,
) -> Vec<Seen> {
    let f: Vec<Seen> = [a, b]
        .iter()
        .map(|n| session_opts(src, tu_steps, reflection, |s, sm| run(s, sm, n)))
        .collect();
    for order in [[a, b], [b, a]] {
        session_opts(src, tu_steps, reflection, |s, sm| {
            for _ in 0..3 {
                for n in order {
                    let want = &f[usize::from(n == b)];
                    assert_eq!(&run(s, sm, n), want, "{n} in order {order:?}");
                }
            }
        });
    }
    f
}

/// Each reference reports nested work (`nested_steps`, `nested_calls`)
/// and, when `warning`, the deprecation warning of `old` (both come from
/// the shared work).
fn assert_nested(f: &[Seen], warning: bool) {
    for x in f {
        assert!(x.nested_steps > 0 && x.nested_calls > 0, "{x:?}");
        assert_eq!(has(x, "'old' is deprecated"), warning, "{x:?}");
    }
}

/// `Class(c)`: a class template specialization's instantiation (here its
/// `static_assert`'s evaluation) shared by two member functions'
/// instantiations, the class required complete by `sizeof` ([temp.inst]/2)
/// and by a member access.
const CLASS_SRC: &str = "constexpr int big(int n) { int s = 0; for (int i = 0; i < n; ++i) s += i; return s; }\n\
template <class T> struct C { static_assert(big(300) + old(T{}) > 0); int x; };\n\
template <class T> struct A { static consteval int f() { return sizeof(C<T>); } };\n\
template <class T> struct B { static consteval int f() { C<T> c{}; return c.x + 1; } };\n\
static_assert(sizeof(A<int>) + sizeof(B<int>) > 0);\n\
using XA = A<int>;\n\
using XB = B<int>;\n";

#[test]
fn a_class_instantiation_is_a_unit() {
    let f = shared_by_two(CLASS_SRC, "XA::f", "XB::f", None);
    assert_nested(&f, true);
}

/// A class completed by a reflection query (`size_of`, [meta.reflection.layout])
/// in one run and by `sizeof` in the other: the query's record reaches
/// the `Class` unit, so a later request for the class through any other
/// unit, or another query, replays it (spec/eval-session.md §7 "Limits").
const META_CLASS_SRC: &str = "namespace std::meta {\n\
using info = decltype(^^int);\n\
consteval decltype(sizeof(0)) size_of(info r);\n\
}\n\
constexpr int big(int n) { int s = 0; for (int i = 0; i < n; ++i) s += i; return s; }\n\
template <class T> struct C { static_assert(big(300) + old(T{}) > 0); int x; };\n\
template <class T> struct A { static consteval int f() { return std::meta::size_of(^^C<T>); } };\n\
template <class T> struct B { static consteval int f() { return sizeof(C<T>) + 1; } };\n\
static_assert(sizeof(A<int>) + sizeof(B<int>) > 0);\n\
using XA = A<int>;\n\
using XB = B<int>;\n";

#[test]
fn a_class_completed_by_a_reflection_query_is_a_unit() {
    let f = shared_by_two_opts(META_CLASS_SRC, "XA::f", "XB::f", None, true);
    assert_nested(&f, true);
    assert_eq!(f[0].result, Ok(RunValue::Signed(4)), "{:?}", f[0]);
}

/// A member class and a member enumeration of a class template
/// specialization ([temp.inst]/3), defined when required.
const MEMBER_CLASS_SRC: &str = "constexpr int big(int n) { int s = 0; for (int i = 0; i < n; ++i) s += i; return s; }\n\
template <class T> struct O {\n\
  struct In { static_assert(big(300) + old(T{}) > 0); int x; };\n\
  enum class E { e = big(200) + old(T{}) };\n\
};\n\
template <class T> struct A { static consteval int f() { return sizeof(typename O<T>::In) + int(O<T>::E::e); } };\n\
template <class T> struct B { static consteval int f() { typename O<T>::In i{}; return i.x + int(O<T>::E::e) + 1; } };\n\
static_assert(sizeof(A<int>) + sizeof(B<int>) > 0);\n\
using XA = A<int>;\n\
using XB = B<int>;\n";

#[test]
fn a_member_class_instantiation_is_a_unit() {
    let f = shared_by_two(MEMBER_CLASS_SRC, "XA::f", "XB::f", None);
    assert_nested(&f, true);
}

/// `Exc(f)`: a function template specialization's `noexcept(e)`
/// instantiated ([except.spec]/13) by a `noexcept` operator in one unit
/// and by a conversion to a pointer to a non-throwing function in the
/// other ([conv.fctptr], [over.ics.rank]), outside `SemaBridge::is_nothrow`.
const EXC_SRC: &str = "constexpr int big(int n) { int s = 0; for (int i = 0; i < n; ++i) s += i; return s; }\n\
template <class T> constexpr int g(T) noexcept(big(300) + old(T{}) > 0) { return 1; }\n\
template <class T> struct A { static consteval bool f() { return noexcept(g(T{})); } };\n\
template <class T> struct B { static consteval int f() { int (*p)(T) noexcept = &g<T>; return p(T{}); } };\n\
static_assert(sizeof(A<int>) + sizeof(B<int>) > 0);\n\
using XA = A<int>;\n\
using XB = B<int>;\n";

#[test]
fn an_exception_specification_is_a_unit() {
    let f = shared_by_two(EXC_SRC, "XA::f", "XB::f", None);
    assert_nested(&f, true);
    assert_eq!(f[0].result, Ok(RunValue::Bool(true)), "{:?}", f[0]);
}

/// The deduced `noexcept` of a defaulted default constructor
/// ([except.spec]/8), which instantiates a member's default member
/// initializer ([class.base.init]/9.1) and the member class's own
/// constructor's specification: asked by a `noexcept` operator in one
/// unit and by a `noexcept` operator on another expression in the other.
const DEFAULTED_SRC: &str = "constexpr int big(int n) { int s = 0; for (int i = 0; i < n; ++i) s += i; return s; }\n\
template <class T> struct M { int v = big(300) + old(T{}); };\n\
template <class T> struct D { M<T> m; D() = default; };\n\
template <class T> struct A { static consteval bool f() { return noexcept(D<T>()); } };\n\
template <class T> struct B { static consteval bool f() { return noexcept(D<T>{}) && noexcept(T{}); } };\n\
static_assert(sizeof(A<int>) + sizeof(B<int>) > 0);\n\
using XA = A<int>;\n\
using XB = B<int>;\n";

#[test]
fn a_deduced_exception_specification_is_a_unit() {
    let f = shared_by_two(DEFAULTED_SRC, "XA::f", "XB::f", None);
    assert_nested(&f, true);
    // `old` is not `noexcept`, so neither is the initializer.
    assert_eq!(f[0].result, Ok(RunValue::Bool(false)), "{:?}", f[0]);
}

/// `Nsdmi(field)`: a default member initializer instantiated at its first
/// use by a constructor ([class.base.init]/9.1, [temp.inst]/3): the
/// defaulted default constructor in one unit, a user-provided one in the
/// other.
const NSDMI_SRC: &str = "constexpr int big(int n) { int s = 0; for (int i = 0; i < n; ++i) s += i; return s; }\n\
template <class T> struct N { int v = big(300) + old(T{}); constexpr N() = default; constexpr N(char) {} };\n\
template <class T> struct A { static consteval int f() { N<T> n; return n.v; } };\n\
template <class T> struct B { static consteval int f() { N<T> n('a'); return n.v + 1; } };\n\
static_assert(sizeof(A<int>) + sizeof(B<int>) > 0);\n\
using XA = A<int>;\n\
using XB = B<int>;\n";

#[test]
fn a_default_member_initializer_is_a_unit() {
    let f = shared_by_two(NSDMI_SRC, "XA::f", "XB::f", None);
    assert_nested(&f, true);
}

/// `Sat(c, args)` and `Atom(e, args)`: a concept's satisfaction
/// ([temp.constr.atomic]/3) needed by two constrained function templates,
/// one through a second concept that names it, and one atom reached
/// directly by two callers with one argument mapping.
const SAT_SRC: &str = "constexpr int big(int n) { int s = 0; for (int i = 0; i < n; ++i) s += i; return s; }\n\
template <class T> concept K = big(sizeof(T) * 100) > 0;\n\
template <class T> concept K2 = K<T> && sizeof(T) > 0;\n\
template <class T> requires K<T> constexpr int ka() { return 1; }\n\
template <class T> requires K2<T> constexpr int kb() { return 2; }\n\
template <class T> struct A { static consteval int f() { return ka<T>(); } };\n\
template <class T> struct B { static consteval int f() { return kb<T>(); } };\n\
static_assert(sizeof(A<int>) + sizeof(B<int>) > 0);\n\
using XA = A<int>;\n\
using XB = B<int>;\n";

#[test]
fn a_constraint_satisfaction_is_a_unit() {
    let f = shared_by_two(SAT_SRC, "XA::f", "XB::f", None);
    assert_nested(&f, false);
}

/// One atom of a requires-clause of a member function of a class template
/// specialization (checked with the class's level and the function's own,
/// so the check itself is not cached; [temp.constr.decl]/3.3), reached by
/// two calls with one argument mapping.
const ATOM_SRC: &str = "constexpr int big(int n) { int s = 0; for (int i = 0; i < n; ++i) s += i; return s; }\n\
template <class T> struct S {\n\
  template <class U> static constexpr int m(U) requires (big(sizeof(T) * 100) > 0) { return 1; }\n\
};\n\
template <class T> struct A { static consteval int f() { return S<T>::m(1); } };\n\
template <class T> struct B { static consteval int f() { return S<T>::m(2) + 1; } };\n\
static_assert(sizeof(A<int>) + sizeof(B<int>) > 0);\n\
using XA = A<int>;\n\
using XB = B<int>;\n";

#[test]
fn an_atomic_constraint_is_a_unit() {
    let f = shared_by_two(ATOM_SRC, "XA::f", "XB::f", None);
    assert_nested(&f, false);
}

/// A kept failure: a constraint whose evaluation exceeds the translation
/// unit's step limit ([expr.const]/2, [temp.constr.atomic]/3: not a
/// constant expression) fails every run that reaches it, through either
/// unit, fresh or used, in either order.
const SAT_LIMIT_SRC: &str = "constexpr int big(int n) { int s = 0; for (int i = 0; i < n; ++i) s += i; return s; }\n\
template <class T> requires (big(sizeof(T) * 25000) > 0) constexpr int k() { return 1; }\n\
template <class T> struct A { static consteval int f() { return k<T>(); } };\n\
template <class T> struct B { static consteval int f() { return k<T>() + 1; } };\n\
static_assert(sizeof(A<int>) + sizeof(B<int>) > 0);\n\
using XA = A<int>;\n\
using XB = B<int>;\n";

#[test]
fn a_constraint_over_the_translation_units_limit_fails_alike() {
    let f = shared_by_two(SAT_LIMIT_SRC, "XA::f", "XB::f", Some(5000));
    for x in &f {
        assert_eq!(x.result, Err(RunError::Failed), "{x:?}");
        assert!(x.nested_steps > 0, "{x:?}");
    }
}
