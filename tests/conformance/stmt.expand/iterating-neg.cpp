// [stmt.expand]/5.2: N is a constant expression (the distance from
// begin-expr to end-expr). A non-constexpr for-range-declaration's range is
// `decltype(auto) range = (s);`, a reference of unknown referent (P2280), so
// a range whose size is a run-time value (a span-like parameter) has no N;
// the note says which read is not constant (a std::array parameter is fine:
// iterating-runtime-range-run.cpp).
//@ expect: compile-fail
//@ strict-diags
//@ requires: clang>=23
//@ diag: 22:24 error "expansion size is not a constant expression"
//@ diag: 17:47 note "read of variable '__range' whose value is not known"
//@ diag: 22:24 note "in call to '__range.begin()'"
//@ diag: 22:26 note "declared here"
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
