// [expr.const]/10.8, [basic.life]/7: a member call through a pointer to a
// parameter whose lifetime ended is undefined. Found by the clang negative-
// test run (WP-912).
//@ std: c++23
//@ xfail(kiln): WP-948 follow-up, by-value parameter objects live to the end of the full-expression, not callee return ([expr.call]/7)
//@ expect: compile-fail
//@ diag: 13:15 error "constexpr variable 'k' must be initialized by a constant expression"
struct Q {
  int n = 0;
  constexpr int f() const { return 0; }
};
constexpr const Q *escape(Q q) { return &q; }
constexpr int k = escape({})->f();
