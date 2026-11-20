// C17 6.5.15/3: the operands of `?:` are arithmetic after lvalue
// conversion, which drops `_Atomic` (6.3.2.1/2), so an atomic operand
// pairs with any arithmetic one (WP-1739: kiln rejected `b ? ai : 1` as
// "incompatible operand types ('int' and 'int')"); an atomic complex arm
// is loaded atomically.
//@ expect: ir
_Atomic int ai;
_Atomic double a2;
_Atomic _Complex double ad;
_Complex double xd;
int f(int b) { return b ? ai : 1; }
double g(int b) { return b ? a2 : 1.0; }
_Complex double h(int b) { return b ? ad : 1.0; }
_Complex double k(int b) { return b ? xd : ad; }
_Complex double m(void) { return ad + xd; }
