// [expr.prim.lambda.capture]: -Wunused-lambda-capture is in -Wunused and
// so in -Wall.
//@ verify
//@ args: -Wall

int f(int x, int y) {
  auto l = [x, y] { return y; }; // expected-warning {{lambda capture 'x' is not used}}
  return l();
}
