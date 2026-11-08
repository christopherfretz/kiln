// [expr.prim.lambda.capture]: -Werror=unused-lambda-capture makes the
// warning an error, which -Wunused alone enables.
//@ verify
//@ args: -Werror=unused-lambda-capture

int f(int x, int y) {
  auto l = [x, y] { return y; }; // expected-error {{lambda capture 'x' is not used}}
  return l();
}
