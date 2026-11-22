// [expr.add]/4, [expr.const]/10.8: pointer arithmetic past the
// one-past-the-end element is undefined behavior, so not a core constant
// expression, also when the evaluation never reads through the pointer
// (the loop leaves before `*p` reaches the end) or moves back in range
// afterwards (`(a + 4) - 2`). clang 19 and 22 alike; the case where a
// later read follows is pointer-past-end-first-not-constant.cpp (WP-1944).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 25:15 error "static assertion expression is not an integral constant expression"
//@ diag: 18:27 note "cannot refer to element 4 of array of 3 elements in a constant expression"
//@ diag: 25:15 note "in call to 'sum_break()'"
//@ diag: 32:15 error "static assertion expression is not an integral constant expression"
//@ diag: 29:14 note "cannot refer to element 4 of array of 3 elements in a constant expression"
//@ diag: 32:15 note "in call to 'back()'"
constexpr int sum_break() {
  int a[3] = {1, 2, 3};
  int s = 0;
  for (int *p = a; p != a + 4; ++p) {
    if (p == a + 3)
      break;
    s += *p;
  }
  return s;
}
static_assert(sum_break() == 6);

constexpr int back() {
  int a[3] = {};
  int *q = a + 4;
  return *(q - 2);
}
static_assert(back() == 0);

// In range: one past the end may be formed and compared.
constexpr int sum_ok() {
  int a[3] = {1, 2, 3};
  int s = 0;
  for (int *p = a; p != a + 3; ++p)
    s += *p;
  return s;
}
static_assert(sum_ok() == 6);
