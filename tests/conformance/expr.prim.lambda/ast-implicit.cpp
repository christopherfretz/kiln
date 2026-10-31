//@ expect: ast
//@ ast-implicit
//@ ast-filter: lamtest
// [expr.prim.lambda.closure]: the closure class holds the inline constexpr
// function call operator, the conversion to pointer to function and the
// static invoker; a referenced closure declares its destructor.
int lamtest(int y) {
  auto l = [](int x) { return x + 1; };
  int (*fp)(int) = l;
  return fp(y) + [] { return 2; }();
}
