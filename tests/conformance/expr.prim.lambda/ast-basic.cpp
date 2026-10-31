//@ expect: ast
// [expr.prim.lambda.general]/2: a LambdaExpr is a prvalue of the closure
// type; its children are the capture initializers and the body.
int lamtest(int y) {
  auto l = [](int x) { return x + 1; };
  auto m = [](int a, int b) noexcept -> long { return a * b; };
  return l(y) + [] { return 2; }() + int(m(1, 2));
}
