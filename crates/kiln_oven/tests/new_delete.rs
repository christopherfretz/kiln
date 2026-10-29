//! Dynamic allocation in constant evaluation ([expr.const]/5.20,
//! [expr.new], [expr.delete]; spec/oven.md "Heap"): scalar and array
//! `new`/`delete`, the clang 19 fault notes, the end-of-evaluation leak
//! check, the `allocs`/peak counters and memoization purity.

mod common;
use common::*;
use kiln_oven::{Budget, EvalRecord, Oven};

fn no_memo() -> Budget {
    Budget { memo_entries: 0, ..Budget::default() }
}

fn run(src: &str, b: Budget) -> (Result<(), Failed>, Vec<EvalRecord>) {
    let mut tu = Tu::new(src);
    let mut oven = Oven::new(b);
    let r = tu.run(&mut oven);
    (r, oven.stats().records().to_vec())
}

/// [expr.new]/[expr.delete]: scalar and array allocations round-trip.
#[test]
fn expr_new_scalar_and_array_roundtrip() {
    ok("\
constexpr int f(int n) {
  int* p = new int(n);
  int r = *p + 1;
  delete p;
  return r;
}
static_assert(f(4) == 5);
constexpr int g(int n) {
  int* a = new int[n]();
  for (int i = 0; i < n; ++i) a[i] += i;
  int s = 0;
  for (int i = 0; i < n; ++i) s += a[i];
  delete[] a;
  return s;
}
static_assert(g(10) == 45);
constexpr int h() {
  int** pp = new int*(nullptr);
  bool ok = *pp == nullptr;
  delete pp;
  return ok;
}
static_assert(h() == 1);
");
}

/// [expr.const]/5.20: storage allocated during an evaluation must be
/// deallocated within it; the note names the first leaked allocation and
/// counts the rest.
#[test]
fn expr_const_5_20_leak() {
    fails_exactly(
        "\
constexpr int f() { new int(1); new int(2); return 0; }
static_assert(f() == 0);
",
        "\
2:15: static assertion expression is not an integral constant expression
1:21: allocation performed here was not deallocated (along with 1 other memory leak)
",
    );
}

/// [expr.const]/5.20: a pointer to a heap allocation is not a permitted
/// result; the note is at the variable (clang's `CheckConstantExpression`).
#[test]
fn expr_const_heap_result() {
    fails_exactly(
        "constexpr int* p = new int(1);\n",
        "\
1:16: constexpr variable must be initialized by a constant expression
1:16: pointer to heap-allocated object is not a constant expression
1:20: heap allocation performed here
",
    );
}

/// [expr.delete]/2-3: the operand faults, in clang 19 wording.
#[test]
fn expr_delete_faults() {
    fails_containing(
        "constexpr int f() { int* p = new int(1); delete p; delete p; return 0; }\nstatic_assert(f() == 0);\n",
        "delete of pointer that has already been deleted",
    );
    fails_containing(
        "constexpr int f() { int* p = new int(1); delete p; return *p; }\nstatic_assert(f() == 0);\n",
        "read of heap allocated object that has been deleted",
    );
    fails_containing(
        "constexpr int f() { int* p = new int[3](); delete p; return 0; }\nstatic_assert(f() == 0);\n",
        "non-array delete used to delete pointer to array object of type 'int[3]'",
    );
    fails_containing(
        "constexpr int f() { int* p = new int(1); delete[] p; return 0; }\nstatic_assert(f() == 0);\n",
        "array delete used to delete pointer to non-array object of type 'int'",
    );
    fails_containing(
        "constexpr int f() { int* p = new int[3](); delete[] (p + 3); return 0; }\nstatic_assert(f() == 0);\n",
        "that does not point to complete object",
    );
    fails_containing(
        "constexpr int f() { int x = 0; delete &x; return 0; }\nstatic_assert(f() == 0);\n",
        "delete of pointer '&x' that does not point to a heap-allocated object",
    );
}

/// [expr.new]/7-8: a negative array bound.
#[test]
fn expr_new_bound_negative() {
    fails_containing(
        "constexpr int f(int n) { int* p = new int[n]; delete[] p; return 0; }\nstatic_assert(f(-1) == 0);\n",
        "cannot allocate array; evaluated array bound -1 is negative",
    );
}

/// [expr.new]: `allocs` counts `new` expressions; peak bytes include the
/// heap.
#[test]
fn expr_new_counters() {
    let src = "\
constexpr int f(int n) {
  int s = 0;
  for (int i = 0; i < n; ++i) { int* p = new int[64](); s += p[63]; delete[] p; }
  return s;
}
static_assert(f(5) == 0);
";
    for b in [no_memo(), Budget::default()] {
        let (r, recs) = run(src, b);
        r.unwrap_or_else(|f| panic!("{}", f.text));
        let rec = recs.last().expect("an evaluation");
        assert_eq!(rec.allocs, 5);
        assert!(rec.peak_bytes >= 64 * 4, "peak {}", rec.peak_bytes);
        assert!(rec.peak_bytes < 2 * 64 * 4 + 256, "freed storage is reused: {}", rec.peak_bytes);
        assert!(rec.peak_arena >= rec.peak_bytes);
    }
}

/// spec/oven.md "Memoization": a call that allocates or deallocates is not
/// memoized. A cached second `leak(1)` would hide its allocation and the
/// note would count no other leak.
#[test]
fn memo_skips_allocating_calls() {
    let src = "\
constexpr int leak(int n) { new int(n); return n; }
static_assert(leak(1) + leak(1) == 2);
";
    for b in [no_memo(), Budget::default()] {
        let f = fails_with(src, b);
        assert!(f.text.contains("(along with 1 other memory leak)"), "{}", f.text);
    }
    let src = "\
constexpr int churn(int n) { int* p = new int(n); int r = *p; delete p; return r; }
static_assert(churn(3) + churn(3) == 6);
";
    let a = run(src, no_memo());
    let b = run(src, Budget::default());
    assert!(a.0.is_ok() && b.0.is_ok());
    let (a, b) = (a.1.last().unwrap().clone(), b.1.last().unwrap().clone());
    assert_eq!((a.allocs, a.calls, a.steps), (2, 2, b.steps));
    assert_eq!((b.allocs, b.calls), (2, 2));
}
