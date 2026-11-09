// [expr.const]/10.8, [expr.add]/4: forming a reference to arr[3] of a
// 3-element array is undefined, so f() is not a constant expression (only
// clang 22 diagnoses it). Found by the clang negative-test run (WP-912).
//@ std: c++20
//@ requires: clang>=22
//@ xfail(kiln): WP-912 follow-up, out-of-bounds reference binding is not detected by the evaluator
//@ expect: compile-fail
//@ diag: 11:16 error "constexpr function never produces a constant expression"
//@ diag: 15:15 error "static assertion expression is not an integral constant expression"
int arr[3];
constexpr bool f() {
  int &r = arr[3];
  return true;
}
static_assert(f(), "");
