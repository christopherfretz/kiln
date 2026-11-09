// [expr.const]/10.8, [basic.life]/7: a member call through a pointer to a
// parameter whose lifetime ended is undefined. Found by the clang negative-
// test run (WP-912). The parameter object's lifetime ends when the call
// returns ([expr.call]/7; clang's choice, WP-1018).
//@ std: c++23
//@ expect: compile-fail
//@ diag: 14:15 error "constexpr variable 'k' must be initialized by a constant expression"
//@ diag: 14:31 note "member call on object outside its lifetime is not allowed in a constant expression"
struct Q {
  int n = 0;
  constexpr int f() const { return 0; }
};
constexpr const Q *escape(Q q) { return &q; }
constexpr int k = escape({})->f();
