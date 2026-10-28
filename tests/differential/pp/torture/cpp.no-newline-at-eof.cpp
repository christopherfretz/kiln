//@ expect: pp
// The main file ends without a trailing newline, inside a macro
// invocation that spans lines: the closing parenthesis is the last byte.
// A function-like macro name not followed by `(` is not invoked.
#define F(x) [x]
before F(1) F F(
  2) F(
  3
)