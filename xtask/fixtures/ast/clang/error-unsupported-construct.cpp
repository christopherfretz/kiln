//@ expect: ast
// A label address (GNU computed goto): clang's AddrLabelExpr is not in
// the mapping table, so the dump cannot be normalized.
void f() { void *p = &&l; l: goto *p; }
