// [stmt.expand]/5.3: a destructuring expansion statement with a
// `constexpr` for-range-declaration declares `constexpr auto&& [u...] =
// expansion-initializer;`, so a prvalue range is a temporary bound by an
// automatic `constexpr` reference, not a copy. Its type is not const, so
// an element that reads it is no constant expression ([expr.const]: a
// read of a temporary outside the expression that created it); a
// for-range-declaration that only refers to it is accepted
// (`destructuring-prvalue-run.cpp`). Found running GCC's C++ testsuite
// (kiln copied the prvalue into a `static constexpr auto`; GCC 16.1
// rejects each statement here).
//@ expect: compile-fail
//@ strict-diags
//@ requires: clang>=23
//@ oracle: gcc
//@ diag: 29:32 error "constexpr variable 'e' must be initialized by a constant expression"
//@ diag: 29:34 note "read of temporary is not allowed in a constant expression outside the expression that created the temporary"
//@ diag: 29:36 note "temporary created here"
//@ diag: 30:31 error "constexpr variable 'e' must be initialized by a constant expression"
//@ diag: 30:33 note "read of temporary is not allowed in a constant expression outside the expression that created the temporary"
//@ diag: 30:35 note "temporary created here"
//@ diag: 31:32 error "constexpr variable 'e' must be initialized by a constant expression"
//@ diag: 31:34 note "read of temporary is not allowed in a constant expression outside the expression that created the temporary"
//@ diag: 31:36 note "temporary created here"
#include <tuple>
struct B { int x = 1, y = 2; };
struct C { const int x = 3; };
int f() {
  int s = 0;
  template for (constexpr auto e : B{}) { s += e; }
  template for (constexpr int e : C{}) { s += e; }
  template for (constexpr auto e : std::tuple<int, long>{4, 5}) { s += e; }
  return s;
}
