// [expr.static.cast]/4, [dcl.init.general]/16.5: `static_cast<T[N]>(e)`
// direct-initializes a temporary array from `(e)`, a parenthesized
// aggregate initialization since C++20 (P1975). Found running GCC's C++
// testsuite (WP-1497).
//@ expect: compile-ok
//@ std: c++20
constexpr int sum() {
  int (&&r)[3] = static_cast<int[3]>(42);
  return r[0] + r[1] + r[2];
}
static_assert(sum() == 42);
int (&&r1)[1] = static_cast<int[1]>(1.3);
struct S { int a; };
constexpr int s() {
  S (&&x)[2] = static_cast<S[2]>(S{5});
  return x[0].a + x[1].a;
}
static_assert(s() == 5);
