// [expr.cond]/7.2: the operands of `?:` are arithmetic after the
// lvalue-to-rvalue conversion, which drops a (clang extension) `_Atomic`
// qualifier, so an atomic operand pairs with any arithmetic one
// (WP-1739: kiln rejected `b ? ai : 1` as "incompatible operand types
// ('int' and 'int')"); an atomic complex arm is loaded atomically.
//@ expect: ir
_Atomic int ai;
_Atomic double ad;
_Atomic _Complex double ac;
_Complex double xc;
int f(int b) { return b ? ai : 1; }
double g(int b) { return b ? ad : ai; }
int h(int b) { return b ? ai : ai; }
_Complex double k(int b) { return b ? ac : xc; }
_Complex double m(int b) { return b ? ac : 2.0; }
