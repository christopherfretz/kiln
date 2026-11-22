//! Inlining of small leaf callees at their call sites (spec/oven.md
//! "Inlining", WP-1924): which calls are inlined, and that nothing an
//! evaluation computes or reports depends on it. Each program runs on
//! two Ovens, inlining on and off; their results, failure notes and
//! per-evaluation records (steps, calls, bytes, memo hits) must be equal.

mod common;
use common::*;
use kiln_oven::{Budget, EvalRecord, Oven};

/// What one run of `src` produced: the failure text or `None`, and the
/// records without their wall times.
fn outcome(src: &str, budget: Budget, inlining: bool) -> (Option<String>, Vec<EvalRecord>, usize) {
    let mut tu = Tu::new(src);
    let mut oven = Oven::new(budget);
    oven.set_inlining(inlining);
    let r = tu.run(&mut oven).err().map(|f| f.text);
    let recs = oven.stats().records().iter().map(|r| EvalRecord { wall_us: 0, ..*r }).collect();
    (r, recs, oven.inlined_sites())
}

/// Runs `src` both ways under `budget`, checks they agree, and returns
/// the failure text (if any) and the sites inlined.
#[track_caller]
fn same(src: &str, budget: Budget) -> (Option<String>, usize) {
    let (on, on_recs, sites) = outcome(src, budget, true);
    let (off, off_recs, none) = outcome(src, budget, false);
    assert_eq!(none, 0, "inlining off inlined a site:\n{src}");
    assert_eq!(on, off, "the failure differs with inlining:\n{src}");
    assert_eq!(on_recs, off_recs, "the records differ with inlining:\n{src}");
    (on, sites)
}

#[track_caller]
fn inlined(src: &str) -> usize {
    let (r, sites) = same(src, Budget::default());
    assert_eq!(r, None, "unexpected failure:\n{src}");
    sites
}

const LEAVES: &str = "
constexpr int get(const int *p) { return *p; }
constexpr int at(const int *p, int i) { return p[i]; }
constexpr int clamp(const int *p, int lo, int hi) {
  if (*p < lo) return lo;
  if (*p > hi) return hi;
  return *p;
}
constexpr void bump(int *p, int by) { *p += by; }
constexpr int count(const int *p) {
  int n = 0;
  for (int i = 0; i < *p; ++i) n += 1;
  return n;
}
";

/// One-return, branching, storing and looping leaves are inlined, each
/// at its own call site.
#[test]
fn leaves_are_inlined_per_site() {
    let src = format!(
        "{LEAVES}
constexpr int f(int n) {{
  int a[4] = {{1, -2, 3, 9}};
  int s = 0;
  for (int i = 0; i < n; ++i) {{
    s += get(&a[i % 4]) + at(a, i % 4) + clamp(&a[i % 4], 0, 5);
    bump(&s, 1);
    s += count(&a[0]);
  }}
  return s;
}}
static_assert(f(8) == 78);
static_assert(f(9) == 83);"
    );
    assert_eq!(inlined(&src), 5);
}

/// A site called once is inlined at that call (the call itself is a
/// real one); a later evaluation runs the copy.
#[test]
fn a_site_is_inlined_at_its_first_call() {
    let src = format!(
        "{LEAVES}
constexpr int once() {{ int x = 7; return get(&x); }}
static_assert(once() == 7);
static_assert(once() == 7);"
    );
    assert_eq!(inlined(&src), 1);
}

/// Memoized callees (scalar parameters), calls in the body, recursion
/// and frame objects keep their calls.
#[test]
fn other_callees_are_not_inlined() {
    for body in [
        // Memoized: the memo answers repeated calls.
        "constexpr int sq(int v) { return v * v; }
         constexpr int f() { int s = 0; for (int i = 0; i < 4; ++i) s += sq(i); return s; }
         static_assert(f() == 14);",
        // Not a leaf.
        "constexpr int sq(int v) { return v * v; }
         constexpr int g(const int *p) { return sq(*p); }
         constexpr int f() { int x = 3; int s = 0; for (int i = 0; i < 4; ++i) s += g(&x); return s; }
         static_assert(f() == 36);",
        // Recursive.
        "constexpr int down(const int *p, int n) { return n == 0 ? *p : down(p, n - 1); }
         constexpr int f() { int x = 2; int s = 0; for (int i = 0; i < 3; ++i) s += down(&x, i); return s; }
         static_assert(f() == 6);",
        // A frame object (a local array).
        "constexpr int two(const int *p) { int b[2] = {*p, *p}; return b[0] + b[1]; }
         constexpr int f() { int x = 2; int s = 0; for (int i = 0; i < 3; ++i) s += two(&x); return s; }
         static_assert(f() == 12);",
    ] {
        assert_eq!(inlined(body), 0, "inlined:\n{body}");
    }
}

/// A fault in an inlined body: the same notes, with the inlined call's
/// "in call to" note and its argument values.
#[test]
fn faults_in_inlined_bodies_keep_their_notes() {
    for (call, want) in [
        ("at(a, i)", "in call to 'at(&a[0], 4)'"),
        ("100 / count(&a[i])", "in call to 'count(&a[4])'"),
        ("clamp(&a[i], 0, 5)", "in call to 'clamp(&a[4], 0, 5)'"),
    ] {
        let src = format!(
            "{LEAVES}
constexpr int f() {{
  int a[4] = {{1, 2, 3, 4}};
  int s = 0;
  for (int i = 0; i < 5; ++i) s += {call};
  return s;
}}
static_assert(f() == 0);"
        );
        let (r, sites) = same(&src, Budget::default());
        let text = r.unwrap_or_else(|| panic!("expected a failure:\n{src}"));
        assert!(text.contains(want), "missing {want:?} in:\n{text}");
        assert!(text.contains("in call to 'f()'"), "missing the caller in:\n{text}");
        assert!(sites >= 1, "not inlined:\n{src}");
    }
}

/// The step limit, at every count around the one the program needs,
/// stops at the same statement with the same notes (inside the inlined
/// bodies too).
#[test]
fn step_limit_is_unchanged() {
    let src = format!(
        "{LEAVES}
constexpr int f() {{
  int a[4] = {{1, 2, 3, 4}};
  int s = 0;
  for (int i = 0; i < 6; ++i) s += get(&a[i % 4]) + count(&a[i % 4]);
  return s;
}}
static_assert(f() == 26);"
    );
    let need = (1..400)
        .find(|&n| same(&src, Budget { steps: n, ..Budget::default() }).0.is_none())
        .expect("the program fits in 400 steps");
    assert!(need > 40, "too few steps to cross the callees: {need}");
}

/// The depth limit counts an inlined call as a level, at the limit and
/// one past it.
#[test]
fn depth_limit_is_unchanged() {
    let src = format!(
        "{LEAVES}
constexpr int deep(const int *p, int n) {{ return get(p) + (n == 0 ? 0 : deep(p, n - 1)); }}
constexpr int one = 1;
static_assert(deep(&one, 18) == 19);
static_assert(deep(&one, 19) == 20);"
    );
    let (r, sites) = same(&src, Budget { depth: 20, ..Budget::default() });
    let text = r.expect("the second assertion exceeds the depth");
    assert!(text.contains("exceeded maximum depth of 20 calls"), "{text}");
    assert!(text.contains("in call to 'deep(&one, 19)'"), "{text}");
    assert_eq!(sites, 1);
    let ok = src.replace("static_assert(deep(&one, 19) == 20);", "");
    assert_eq!(same(&ok, Budget { depth: 20, ..Budget::default() }).0, None);
}

/// With the memo off, a scalar one-return function is a leaf like any
/// other: inlined, with the same counts.
#[test]
fn scalar_leaves_inline_without_the_memo() {
    let src = "constexpr int sq(int v) { return v * v; }
         constexpr int f() { int s = 0; for (int i = 0; i < 4; ++i) s += sq(i); return s; }
         static_assert(f() == 14);";
    let (r, sites) = same(src, Budget { memo_entries: 0, ..Budget::default() });
    assert_eq!(r, None);
    assert_eq!(sites, 1);
}
