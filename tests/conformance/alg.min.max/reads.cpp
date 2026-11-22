// [alg.min.max], [expr.const]/10.9: an argument that cannot be read
// makes the call no constant expression. The Oven's native std::min
// (spec/oven.md "Library fast paths") declines and the library's code
// runs, so the notes are the call's.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 19:15 error "static assertion expression is not an integral constant expression"
//@ diag: 17:10 note "in call to 'min<int>(a, b)'"
//@ diag: 19:15 note "in call to 'pick()'"
//@ diag: 26:15 error "static assertion expression is not an integral constant expression"
//@ diag: 24:10 note "read of object outside its lifetime is not allowed in a constant expression"
//@ diag: 26:15 note "in call to 'dangle()'"
#include <algorithm>
constexpr int pick() {
  int a;
  int b = 2;
  return std::min(a, b);
}
static_assert(pick() == 2);
// The result refers to a temporary destroyed at the end of the
// full-expression, as the library's does.
constexpr int dangle() {
  const int &r = std::max(1, 2);
  return r;
}
static_assert(dangle() == 2);
