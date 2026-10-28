//@ expect: pp
// A macro name immediately followed by `(` defines a function-like macro;
// with whitespace before `(` it is object-like and the parenthesis is
// part of the replacement. A function-like name not followed by `(` is
// not invoked.
#define OBJ (x) + 1
#define FN(x) x + 1
#define FN0() zero
OBJ OBJ(2) FN(3) FN FN0() FN0 ;
#define PARENS() ()
PARENS() PARENS
#define SELF SELF + 1
#define MUTUAL_A MUTUAL_B a
#define MUTUAL_B MUTUAL_A b
SELF MUTUAL_A MUTUAL_B
