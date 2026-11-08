// [expr.prim.lambda.capture]: -Wunused-lambda-capture is off by default.
//@ verify
// expected-no-diagnostics

int f(int x, int y) {
  auto l = [x, y] { return y; };
  return l();
}
