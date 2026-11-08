// GNU extension: an `__attribute__((...))` list may precede a statement
// that is not a declaration (an expression statement, `return`, the
// body of an `if`); clang `ParseStatementOrDeclaration` attaches it to
// the statement (`nomerge`, `musttail`, ...). Found writing WP-805's
// class-A tests (`__attribute__((musttail)) return g(x);`).
//@ expect: compile-ok
//@ strict-diags
//@ xfail(kiln): WP-805 follow-up, GNU attributes are parsed before a statement only when a declaration or `;` follows ("expected expression")
int g(int);
int f(int x) {
  __attribute__((nomerge)) g(x);
  if (x)
    __attribute__((nomerge)) return g(x);
  __attribute__((nomerge)) return g(x + 1);
}
