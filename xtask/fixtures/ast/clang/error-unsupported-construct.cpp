//@ expect: ast
// A labeled statement: clang's LabelStmt is not in the mapping table, so
// the dump cannot be normalized.
void f() { l: goto l; }
