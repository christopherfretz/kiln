// [expr.const]/10.8, [expr.add]/4: forming a reference to arr[3] of a
// 3-element array is undefined, so f() is not a constant expression (only
// clang 22 diagnoses it). Found by the clang negative-test run (WP-912).
//@ std: c++20
//@ requires: clang>=22
//@ expect: compile-fail
//@ diag: 13:16 error "constexpr function never produces a constant expression"
//@ diag: 14:12 note "read of dereferenced one-past-the-end pointer is not allowed in a constant expression"
//@ diag: 17:15 error "static assertion expression is not an integral constant expression"
//@ diag: 14:12 note "read of dereferenced one-past-the-end pointer is not allowed in a constant expression"
//@ diag: 17:15 note "in call to 'f()'"
int arr[3];
constexpr bool f() {
  int &r = arr[3];
  return true;
}
static_assert(f(), "");
