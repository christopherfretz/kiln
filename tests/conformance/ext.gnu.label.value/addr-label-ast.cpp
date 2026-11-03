// GNU labels as values (an extension; [stmt.label]): `&&label` is an
// `AddrLabelExpr` prvalue of type `void *` naming the label, which may be
// defined later in the function (WP-424).
//@ expect: ast
void f(int x) {
a:
  void *p = &&a;
  void *tab[] = { &&a, &&b };
  bool c = x && &&b != p;
b:;
}
