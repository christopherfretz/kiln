// [dcl.pre]/P3289R1, [stmt.pre]: a consteval-block-declaration is a
// block-declaration; at block scope it is evaluated once, at translation
// time, and never at run time (it is not codegen'd: the program runs the
// enclosing function without calling rt).
//@ expect: run
//@ oracle: none
//@ skip(clang): clang 19 has no consteval blocks (P3289R1)
//@ strict-diags
int rt();
consteval int twice(int x) { return 2 * x; }
int f(int n) {
  int r = n;
  consteval {
    int k = twice(3);
    for (int i = 0; i < 3; ++i) k -= 2;
    if (k != 0) rt();
  }
  if (r > 0) {
    consteval { static_assert(sizeof(int) >= 2); }
  }
  return r;
}
// In a lambda body and in a constexpr function (not evaluated by calls).
auto lam = [](int x) {
  consteval { if (twice(1) != 2) rt(); }
  return x;
};
constexpr int g(int x) {
  consteval { if (twice(2) != 4) rt(); }
  return x + 1;
}
static_assert(g(1) == 2);
int main() { return f(0) + lam(0) + g(-1); }
