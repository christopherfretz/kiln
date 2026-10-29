// [expr.const]/5.20: storage allocated by a new-expression during a constant
// evaluation must be deallocated within that evaluation. The note names the
// first leaked allocation and counts the others. A call that allocates is not
// memoized (spec/oven.md "Memoization"), so the two calls of leak() leak
// twice.
// Mirrors crates/kiln_oven/tests/new_delete.rs `expr_const_5_20_leak` and
// `memo_skips_allocating_calls`.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 20:15 error "static assertion expression is not an integral constant expression"
//@ diag: 17:12 note "allocation performed here was not deallocated"
//@ diag: 27:15 error "static assertion expression is not an integral constant expression"
//@ diag: 23:3 note "allocation performed here was not deallocated (along with 1 other memory leak)"
//@ diag: 36:15 error "static assertion expression is not an integral constant expression"
//@ diag: 30:12 note "allocation performed here was not deallocated (along with 1 other memory leak)"
constexpr int one() {
  int *p = new int(1);
  return *p;
}
static_assert(one() == 1);

constexpr int leak(int n) {
  new int(n);
  return n;
}
// Two calls, two leaks: a call that allocates is never memoized.
static_assert(leak(1) + leak(1) == 2);

constexpr int some(int n) {
  int *a = new int[n]();
  int *b = new int(2);
  int *c = new int[3]{};
  delete b;
  return a[0] + c[0];
}
static_assert(some(4) == 0);
