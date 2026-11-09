// [dcl.constexpr]/6 since P2448 (C++23): a constexpr function that is never
// constant is valid, and clang 19 does not check it unless
// -Winvalid-constexpr is given (never-constant-builtin-cxx23.cpp): a call
// of a builtin it does not evaluate is no error by default (WP-945).
// spec/sema.md "Never constant".
//@ std: c++23
//@ strict-diags
//@ expect: compile-ok
constexpr double w1(double x) { return x + __builtin_floor(1.0); }
extern "C" double fabs(double);
constexpr double w2(double x) { return fabs(2.0) + x; }
constexpr int w3(int x) {
  asm("");
  return x;
}
