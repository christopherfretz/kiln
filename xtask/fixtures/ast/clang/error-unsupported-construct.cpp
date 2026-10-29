//@ expect: ast
// A throw expression: clang's CXXThrowExpr is not in the mapping table, so
// the dump cannot be normalized.
void f() { throw 1; }
