// [expr.add]/4, [expr.const]/10.8: `a + 4` over `int a[3]` would point past
// the one-past-the-end element, which is undefined behavior, so an
// evaluation that computes it is not a core constant expression. The
// loop's condition `p != a + 4` computes it in the first iteration
// ([stmt.for]/1), before the read of the one-past-the-end element `*p`
// ([expr.const]/10.9) in the fourth: kiln reports the first operation that
// is not constant. clang 19 and 22 make the pointer arithmetic a
// "not a core constant expression" note that a later failure replaces,
// and report the read (docs/status/deviations.md; WP-1944). Without such
// a read they report `a + 4` as kiln does:
// pointer-past-end-not-constant.cpp.
//@ expect: compile-fail
//@ skip(clang): clang 19 and 22 report the later one-past-the-end read, not the first operation that is not constant (docs/status/deviations.md)
//@ strict-diags
//@ diag: 25:15 error "static assertion expression is not an integral constant expression"
//@ diag: 21:27 note "cannot refer to element 4 of array of 3 elements in a constant expression"
//@ diag: 25:15 note "in call to 'sum()'"
constexpr int sum() {
  int a[3] = {1, 2, 3};
  int s = 0;
  for (int *p = a; p != a + 4; ++p)
    s += *p;
  return s;
}
static_assert(sum() == 6);
