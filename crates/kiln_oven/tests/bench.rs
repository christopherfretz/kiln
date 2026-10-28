//! Benchmarks (spec/oven.md "Performance"): `cargo test --release -p
//! kiln_oven -- --ignored bench --nocapture`. Each prints the evaluation's
//! steps, allocated and peak abstract-memory bytes, the evaluator's heap
//! footprint and the median wall time of repeated evaluations of the same
//! (already compiled) expression. [implimits]: steps are clang's.

mod common;
use common::*;
use kiln_oven::{Budget, Oven};
use std::time::{Duration, Instant};

fn bench(name: &str, src: &str, budget: Budget, reps: usize) {
    let mut tu = Tu::new(src);
    let Some(&Item::Assert { cond, loc }) =
        tu.items.iter().find(|i| matches!(i, Item::Assert { .. }))
    else {
        panic!("no static_assert")
    };
    let mut oven = Oven::new(budget);
    let t0 = Instant::now();
    let v = tu.eval(&mut oven, cond, loc).unwrap_or_else(|f| panic!("{}", tu.render(&f)));
    let first = t0.elapsed();
    assert!(matches!(
        tu.ast.consts.scalar(v),
        Some(kiln_ast::consts::Scalar::Bool { value: true, .. })
    ));
    let mut times: Vec<Duration> = (0..reps)
        .map(|_| {
            let t = Instant::now();
            tu.eval(&mut oven, cond, loc).expect("ok");
            t.elapsed()
        })
        .collect();
    times.sort();
    let r = *oven.stats().last().expect("record");
    let med = times[times.len() / 2];
    #[allow(clippy::cast_precision_loss, reason = "a rate for display")]
    let rate = r.steps as f64 / med.as_secs_f64() / 1e6;
    println!(
        "bench {name}: steps={} calls={} bytes={} peak_bytes={} footprint={} first={first:?} median={med:?} ({rate:.0} Msteps/s)",
        r.steps,
        r.calls,
        r.bytes,
        r.peak_bytes,
        oven.footprint()
    );
}

/// Without the cache (a zero-argument call is itself memoized, so repeat
/// evaluations would measure a cache hit).
const NO_MEMO: Budget = Budget { steps: 1_048_576, depth: 512, bytes: 256 << 20, memo_entries: 0 };

const FIB: &str = "
constexpr int fib(int n) { return n < 2 ? n : fib(n - 1) + fib(n - 2); }
static_assert(fib(25) == 75025);
";

#[test]
#[ignore = "benchmark"]
fn bench_fib25() {
    bench("fib(25)", FIB, NO_MEMO, 21);
}

#[test]
#[ignore = "benchmark"]
fn bench_fib25_memo() {
    bench("fib(25) memo", FIB, Budget::default(), 201);
}

#[test]
#[ignore = "benchmark"]
fn bench_sieve10k() {
    bench(
        "sieve(10000)",
        "
constexpr int sieve() {
  bool composite[10001] = {};
  int count = 0;
  for (int i = 2; i <= 10000; ++i) {
    if (composite[i])
      continue;
    ++count;
    for (int j = i * i; j <= 10000; j += i)
      composite[j] = true;
  }
  return count;
}
static_assert(sieve() == 1229);
",
        NO_MEMO,
        101,
    );
}

#[test]
#[ignore = "benchmark"]
fn bench_sort1k_structs() {
    bench(
        "sort(1000 structs)",
        "
struct Rec { int key; int id; double w; };
constexpr void sift(Rec *a, int start, int end) {
  int root = start;
  while (2 * root + 1 < end) {
    int child = 2 * root + 1;
    if (child + 1 < end && a[child].key < a[child + 1].key)
      ++child;
    if (a[root].key >= a[child].key)
      return;
    Rec t = a[root];
    a[root] = a[child];
    a[child] = t;
    root = child;
  }
}
constexpr bool sorted() {
  Rec a[1000] = {};
  unsigned x = 12345;
  for (int i = 0; i < 1000; ++i) {
    x = x * 1103515245u + 12345u;
    a[i].key = (int)(x >> 16) % 10007;
    a[i].id = i;
    a[i].w = i * 0.5;
  }
  for (int s = 1000 / 2 - 1; s >= 0; --s)
    sift(a, s, 1000);
  for (int end = 999; end > 0; --end) {
    Rec t = a[0];
    a[0] = a[end];
    a[end] = t;
    sift(a, 0, end);
  }
  for (int i = 1; i < 1000; ++i)
    if (a[i - 1].key > a[i].key)
      return false;
  return true;
}
static_assert(sorted());
",
        NO_MEMO,
        21,
    );
}
