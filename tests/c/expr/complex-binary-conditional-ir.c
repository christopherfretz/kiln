// GNU `x ?: y` with complex operands (WP-1739): the first operand is
// emitted once, as a complex value, and both of its parts are compared
// with zero (clang `EvaluateExprAsBool` of a complex, the operand bound
// as an opaque value). In C the operand is converted to a value first
// (clang `UsualUnaryConversions`), so a volatile one is read once.
//@ expect: ir
_Complex double g(void);
_Complex float h(_Complex float x, _Complex float y) { return x ?: y; }
_Complex double k(_Complex double y) { return g() ?: y; }
_Complex int m(_Complex int *p, _Complex int y) { return *p ?: y; }
volatile int vi;
int n(int y) { return vi ?: y; }
