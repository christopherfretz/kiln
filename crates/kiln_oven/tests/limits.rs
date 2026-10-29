//! Limits, memoization, metrics and the report ([expr.const]/5.7,
//! [implimits]; spec/oven.md "Limits and metrics", "Memoization",
//! "Reporting protocol"). Step and depth goldens are the smallest
//! `-fconstexpr-steps=` / `-fconstexpr-depth=` clang 19 accepts for the
//! same source.

mod common;
use common::*;
use kiln_oven::{Budget, EvalRecord, Oven, report};

const LOOP: &str = "\
constexpr int f(int n) { int s = 0; for (int i = 0; i < n; ++i) s += i; return s; }
static_assert(f(10) == 45);
";
const FIB: &str = "\
constexpr int fib(int n) { return n < 2 ? n : fib(n - 1) + fib(n - 2); }
static_assert(fib(10) == 55);
";
const STMTS: &str = "\
constexpr int g(int x) {
  int a = x;
  if (a > 2) { a = a * 2; } else { a = 0; }
  while (a > 0) { a -= 3; if (a == 1) break; }
  do { ++a; } while (a < 5);
  switch (a) { case 5: a = 7; break; default: a = 1; }
  return a;
}
static_assert(g(10) == 7);
";
const CHAIN: &str = "\
constexpr int f(int n) { return n == 0 ? 0 : 1 + f(n - 1); }
constexpr int g() { return f(30); }
static_assert(g() == 30);
";

fn no_memo() -> Budget {
    Budget { memo_entries: 0, ..Budget::default() }
}

fn run(src: &str, b: Budget) -> (Result<(), Failed>, Vec<EvalRecord>) {
    let mut tu = Tu::new(src);
    let mut oven = Oven::new(b);
    let r = tu.run(&mut oven);
    (r, oven.stats().records().to_vec())
}

/// The last evaluation's steps, and that exactly that many is the
/// smallest step budget that succeeds.
fn steps_exact(src: &str, b: Budget) -> u64 {
    let (r, recs) = run(src, b);
    r.unwrap_or_else(|f| panic!("{}", f.text));
    let n = recs.last().expect("an evaluation").steps;
    assert!(run(src, Budget { steps: n, ..b }).0.is_ok(), "budget {n} must suffice");
    if n > 0 {
        let f = run(src, Budget { steps: n - 1, ..b }).0.expect_err("one step short");
        assert!(f.text.contains("constexpr evaluation hit maximum step limit"), "{}", f.text);
    }
    n
}

/// [implimits]: statements are counted as clang counts them (one step per
/// statement executed, loop bodies per iteration).
#[test]
fn implimits_steps_match_clang() {
    for b in [no_memo(), Budget::default()] {
        assert_eq!(steps_exact(LOOP, b), 15);
        assert_eq!(steps_exact(FIB, b), 354);
        assert_eq!(steps_exact(STMTS, b), 46);
        assert_eq!(steps_exact("static_assert(1 + 2 == 3);\n", b), 0);
        assert_eq!(steps_exact("constexpr int h() { return 4; }\nconstexpr int v = h();\n", b), 2);
    }
}

/// [expr.const]/5.7: the step-limit note, at the statement that would
/// exceed the budget.
#[test]
fn expr_const_5_7_step_limit_note() {
    let f = fails_with(LOOP, Budget { steps: 9, ..no_memo() });
    assert_eq!(
        f.text,
        "2:15: static assertion expression is not an integral constant expression\n\
         1:65: constexpr evaluation hit maximum step limit; possible infinite loop?\n\
         2:15: in call to 'f(10)'\n"
    );
}

/// [implimits]: the depth limit counts active calls, as clang's
/// `-fconstexpr-depth`.
#[test]
fn implimits_depth_match_clang() {
    for b in [no_memo(), Budget::default()] {
        for (src, need) in [(FIB, 10), (CHAIN, 32), ("static_assert(1 + 2 == 3);\n", 0)] {
            assert!(run(src, Budget { depth: need, ..b }).0.is_ok(), "depth {need}:\n{src}");
            if need > 0 {
                let f = run(src, Budget { depth: need - 1, ..b }).0.expect_err("one short");
                assert!(f.text.contains("exceeded maximum depth of"), "{}", f.text);
            }
        }
    }
}

/// [expr.const]/5.7: the depth note and its elided backtrace (clang 19,
/// `-fconstexpr-depth=20`).
#[test]
fn expr_const_5_7_depth_limit_note() {
    let f = fails_with(CHAIN, Budget { depth: 20, ..Budget::default() });
    assert_eq!(
        f.text,
        "3:15: static assertion expression is not an integral constant expression
1:50: constexpr evaluation exceeded maximum depth of 20 calls
1:50: in call to 'f(12)'
1:50: in call to 'f(13)'
1:50: in call to 'f(14)'
1:50: in call to 'f(15)'
1:50: in call to 'f(16)'
1:50: (skipping 10 calls in backtrace; use -fconstexpr-backtrace-limit=0 to see all)
1:50: in call to 'f(27)'
1:50: in call to 'f(28)'
1:50: in call to 'f(29)'
2:28: in call to 'f(30)'
3:15: in call to 'g()'
"
    );
}

/// [expr.const]/5.7 (kiln extension `-fconstexpr-memory`): live abstract
/// memory is bounded.
#[test]
fn implimits_memory_budget() {
    let src = "\
constexpr int f() {
  int a[4096] = {};
  a[4095] = 1;
  return a[4095];
}
static_assert(f() == 1);
";
    let (r, recs) = run(src, Budget::default());
    assert!(r.is_ok());
    let peak = recs[0].peak_bytes;
    assert!(peak >= 4 * 4096, "peak {peak}");
    assert!(run(src, Budget { bytes: peak, ..Budget::default() }).0.is_ok());
    let f = fails_with(src, Budget { bytes: 8192, ..Budget::default() });
    assert_eq!(
        f.text,
        "6:15: static assertion expression is not an integral constant expression\n\
         2:7: constexpr evaluation exceeded the memory limit of 8192 bytes\n\
         6:15: in call to 'f()'\n"
    );
}

/// Memoization is unobservable: a hit charges the steps, calls, bytes,
/// peak and depth the call cost when it ran, so every counter and every
/// budget decision is the same with the cache on or off.
#[test]
fn memo_hits_charge_recorded_costs() {
    let src = "\
constexpr int fib(int n) { return n < 2 ? n : fib(n - 1) + fib(n - 2); }
static_assert(fib(20) == 6765);
static_assert(fib(20) + fib(19) == 10946);
constexpr int f(int n) { return n == 0 ? 0 : 1 + f(n - 1); }
static_assert(f(30) == 30);
constexpr int g() { return f(30); }
static_assert(g() == 30);
";
    let (a, ra) = run(src, Budget::default());
    let (b, rb) = run(src, no_memo());
    assert!(a.is_ok() && b.is_ok());
    let strip =
        |r: &[EvalRecord]| r.iter().map(|r| EvalRecord { wall_us: 0, ..*r }).collect::<Vec<_>>();
    assert_eq!(strip(&ra), strip(&rb));
    // `f(30)` is cached by the third assertion; the fourth still needs 32
    // frames.
    for budget in [Budget { depth: 31, ..Budget::default() }, Budget { depth: 31, ..no_memo() }] {
        let f = run(src, budget).0.expect_err("depth 31");
        assert!(f.text.starts_with("7:15: "), "{}", f.text);
    }
    // And the same step budget fails at the same evaluation.
    assert!(ra[1].steps > ra[0].steps);
    for budget in [Budget::default(), no_memo()] {
        let b = Budget { steps: ra[1].steps - 1, ..budget };
        let f = run(src, b).0.expect_err("short");
        assert!(f.text.starts_with("3:15: "), "{}", f.text);
    }
}

/// Only pure calls are cached: a call that writes through a reference or
/// reads through a pointer argument is re-executed.
#[test]
fn memo_purity() {
    ok("
constexpr void inc(int *x) { ++*x; }
constexpr int twice() {
  int a = 0;
  inc(&a);
  inc(&a);
  return a;
}
static_assert(twice() == 2);
static_assert(twice() == 2);
constexpr int get(const int *p) { return *p; }
constexpr int sum() {
  int a = 1, b = 2;
  return get(&a) + get(&b) + get(&a);
}
static_assert(sum() == 4);
constexpr int set_then_get(int v) {
  int a = 0;
  int *p = &a;
  *p = v;
  return get(p);
}
static_assert(set_then_get(3) + set_then_get(4) == 7);
");
}

/// spec/oven.md "Reporting protocol": one NDJSON line per evaluation
/// (`try_fold` and constant-initialization checks are not recorded), then
/// the totals.
#[test]
fn report_golden() {
    let src = "\
constexpr int fib(int n) { return n < 2 ? n : fib(n - 1) + fib(n - 2); }
static_assert(fib(5) == 5);
constexpr int v = fib(6);
const int k = fib(3);
static_assert(v + k == 10);
";
    let mut tu = Tu::new(src);
    let mut oven = Oven::new(Budget::default());
    tu.run(&mut oven).expect("ok");
    let loc = |l| Some(format!("t.cpp:{}", tu.line_col(l)));
    let mut out = report::render(oven.stats(), &loc);
    // Wall time is the one nondeterministic field.
    while let Some(i) = out.find("\"wall_us\":") {
        let j = i + out[i..].find([',', '}']).expect("end");
        out.replace_range(i..j, "\"wall_us\"=0");
    }
    let out = out.replace("\"wall_us\"=0", "\"wall_us\":0");
    assert_eq!(
        out,
        r#"{"loc":"t.cpp:2:15","kind":"static_assert","steps":30,"bytes":0,"peak_bytes":0,"calls":15,"allocs":0,"peak_arena":0,"frozen_bytes":16,"wall_us":0}
{"loc":"t.cpp:3:15","kind":"init","steps":50,"bytes":0,"peak_bytes":0,"calls":25,"allocs":0,"peak_arena":0,"frozen_bytes":16,"wall_us":0}
{"loc":"t.cpp:5:15","kind":"static_assert","steps":0,"bytes":0,"peak_bytes":0,"calls":0,"allocs":0,"peak_arena":0,"frozen_bytes":0,"wall_us":0}
{"total":{"steps":80,"bytes":0,"peak_bytes":0,"calls":40,"allocs":0,"peak_arena":0,"frozen_bytes":32,"wall_us":0,"evaluations":3}}
"#
    );
    let path = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("kiln_oven_report.ndjson");
    report::write(oven.stats(), &|_| None, &path).expect("write");
    let written = std::fs::read_to_string(&path).expect("read");
    std::fs::remove_file(&path).expect("remove");
    assert!(written.starts_with("{\"loc\":null,\"kind\":\"static_assert\""), "{written}");
    assert_eq!(written.lines().count(), 4);
}

/// spec/oven.md "Memory": the frame arena is reset, not freed, between
/// evaluations, so 10k evaluations of the same expression hold the same
/// heap as the first few and record identical counters.
#[test]
fn memory_reset_peak_stable() {
    let src = "\
struct P { int x; double y; };
constexpr int f(int n) {
  P ps[64] = {};
  int a[256] = {};
  for (int i = 0; i < n; ++i) {
    a[i % 256] += i;
    ps[i % 64].x = a[i % 256];
  }
  return ps[n % 64].x;
}
static_assert(f(300) == 21);
";
    let mut tu = Tu::new(src);
    let Item::Assert { cond, loc } = tu.items[0] else { panic!("assert") };
    let mut oven = Oven::new(Budget::default());
    for _ in 0..16 {
        tu.eval(&mut oven, cond, loc).expect("ok");
    }
    let warm = oven.footprint();
    let first = *oven.stats().last().expect("record");
    for _ in 0..10_000 {
        tu.eval(&mut oven, cond, loc).expect("ok");
    }
    assert_eq!(oven.footprint(), warm, "no growth after warm-up");
    let t = oven.stats().totals();
    assert_eq!(t.evaluations, 10_016);
    assert_eq!(t.peak_bytes, first.peak_bytes);
    for r in oven.stats().records() {
        assert_eq!((r.steps, r.bytes, r.peak_bytes), (first.steps, first.bytes, first.peak_bytes));
    }
}
