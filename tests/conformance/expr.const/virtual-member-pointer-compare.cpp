// [expr.eq]/4.3, [expr.const]/10.24: comparing pointers to virtual member
// functions is unspecified, so not a constant expression. Found by the clang
// negative-test run (WP-912).
//@ std: c++23
//@ expect: compile-fail
//@ diag: 12:15 error "static assertion expression is not an integral constant expression"
//@ diag: 12:18 note "comparison of pointer to virtual member function 'g' has unspecified value"
struct S {
  virtual void g();
};
constexpr void (S::*pg)() = &S::g;
static_assert(pg == pg, "");
