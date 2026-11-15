// [stmt.expand]/5.2: N of an iterating expansion statement is a constant
// expression (the distance from begin-expr to end-expr, evaluated in a
// consteval lambda). A range whose size is a run-time value (a span-like
// parameter) has none, and the range is not a constant expression either,
// so the first attempt's diagnostic stands (the constexpr range kiln tries
// first, then the non-constexpr range of CWG3140) and the statement is
// dropped. A std::array parameter is fine (iterating-runtime-range-run.cpp).
//@ expect: compile-fail
//@ strict-diags
//@ requires: clang>=23
//@ diag: 22:26 error "constexpr variable '__range' must be initialized by a constant expression"
//@ diag: 22:26 note "reference to 's' is not a constant expression"
//@ diag: 20:14 note "declared here"
struct Span {
  const int *p;
  int n;
  constexpr const int *begin() const { return p; }
  constexpr const int *end() const { return p + n; }
};
int sum(Span s) {
  int t = 0;
  template for (auto x : s) { t += x; }
  return t;
}
