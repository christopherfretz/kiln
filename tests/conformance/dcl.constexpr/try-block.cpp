// [dcl.constexpr]/3 (P1002R1): a constexpr function may contain a try-block;
// constant evaluation runs the compound statement and never a handler
// ([expr.const]/10: a throw-expression, even one a handler would catch, is
// not a core constant expression). Matches clang 19 EvaluateStmt.
// Pinned to C++23: in C++26 a caught or uncaught throw is evaluated (P3068).
//@ std: c++23
//@ expect: compile-fail
//@ strict-diags
//@ diag: 45:15 error "static assertion expression is not an integral constant expression"
//@ diag: 38:12 note "subexpression not valid in a constant expression"
//@ diag: 45:15 note "in call to 'c(2)'"
constexpr int a(int x) {
  try {
    int y = x * 2;
    return y + 1;
  } catch (int e) {
    return e;
  } catch (...) {
    return -1;
  }
}
static_assert(a(3) == 7);
constexpr int b(int x) {
  int r = 0;
  for (int i = 0; i < x; ++i) {
    try { try { r += i; } catch (long) { r = -100; } } catch (...) { return -1; }
  }
  return r;
}
static_assert(b(5) == 10);
struct S {
  int v;
  constexpr S(int x) try : v(x) { } catch (...) { }
};
static_assert(S(4).v == 4);
constexpr int c(int x) {
  try {
    if (x) throw x;
  } catch (int e) {
    return e;
  }
  return 0;
}
static_assert(c(0) == 0);
static_assert(c(2) == 2);
