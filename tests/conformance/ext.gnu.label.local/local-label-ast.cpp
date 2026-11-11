// GNU local labels: `__label__ a, b;` is a DeclStmt of LabelDecls that the
// block's goto, `&&` and label statements name ([stmt.label]).
//@ expect: ast
void f(int x) {
  {
    __label__ a, b;
    if (x) goto a;
  a:
    void *p = &&b;
  b:;
  }
}
