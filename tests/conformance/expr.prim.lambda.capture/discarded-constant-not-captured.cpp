// [basic.def.odr]/5: a variable usable in constant expressions that is an
// element of the potential results of a discarded-value expression is not
// odr-used by it, so a capture-default does not capture it: an expression
// statement `n;`, the operand of a cast to void ([expr.static.cast]/6) and
// the left operand of a comma ([expr.comma]/1), also through parentheses
// and a conditional. A volatile variable is not usable in constant
// expressions and stays captured. Found running GCC's C++ testsuite; clang
// 19 and 22 odr-use (and capture) the variable: the text and GCC 16 decide.
//@ xfail(clang): clang 19 and 22 capture a constant named only by a discarded-value expression
//@ expect: compile-ok
//@ args: -Wno-unused-value
void g(bool b) {
  const int n = 0;
  constexpr int m = 1;
  auto x = [=] { n; };
  static_assert(sizeof(x) == 1);
  auto y = [=] { (void)n; (n); };
  static_assert(sizeof(y) == 1);
  auto z = [=] { n, m; return 0; };
  static_assert(sizeof(z) == 1);
  auto w = [=] { b ? n : m; };
  static_assert(sizeof(w) == sizeof(bool));
  volatile const int v = 0;
  auto u = [=] { v; };
  static_assert(sizeof(u) == sizeof(int));
}
