// [expr.prim.lambda.capture]: -Wno-unused-lambda-capture turns it off
// under -Wall -Wextra; it is off by default.
//@ verify
//@ args: -Wall -Wextra -Wno-unused-lambda-capture
// expected-no-diagnostics

int f(int x, int y) {
  auto l = [x, y] { return y; };
  return l();
}
