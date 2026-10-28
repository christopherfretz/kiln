//@ expect: ast
// A class-type default initialization: clang's CXXConstructExpr is not in
// the M2 mapping table, so the dump cannot be normalized.
struct P {
  P();
};
void f() { P p; }
