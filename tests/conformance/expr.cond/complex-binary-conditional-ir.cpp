// GNU `x ?: y` with complex operands (WP-1739): the first operand is
// emitted once, as a complex value bound to the opaque operand, and is
// the result when either part is nonzero (clang
// `ComplexExprEmitter::VisitBinaryConditionalOperator`).
//@ expect: ir
_Complex double g();
_Complex float h(_Complex float x, _Complex float y) { return x ?: y; }
_Complex double k(_Complex double y) { return g() ?: y; }
_Complex int m(_Complex int &r, _Complex int y) { return r ?: y; }
