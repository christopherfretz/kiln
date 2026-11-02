// [stmt.expr]/1, [expr.sub]: an unused built-in subscript is reported at its
// `]` (clang `isUnusedResultAWarning`, `getRBracketLoc`), also as a comma's
// left operand and inside parentheses; other unused forms keep their own
// locations (`*p` at the `*`, `s.b` at the member name, `-a[i]` at the
// operator) and `(void)0` is not warned.
//@ strict-diags
//@ diag: 27:6 warning "expression result unused [-Wunused-value]"
//@ diag: 28:6 warning "expression result unused [-Wunused-value]"
//@ diag: 29:3 warning "expression result unused [-Wunused-value]"
//@ diag: 30:5 warning "expression result unused [-Wunused-value]"
//@ diag: 31:6 warning "expression result unused [-Wunused-value]"
//@ diag: 33:8 warning "expression result unused [-Wunused-value]"
//@ diag: 34:10 warning "expression result unused [-Wunused-value]"
//@ diag: 35:7 warning "expression result unused [-Wunused-value]"
//@ diag: 36:3 warning "expression result unused [-Wunused-value]"
//@ diag: 37:3 warning "expression result unused [-Wunused-value]"
//@ diag: 38:3 warning "expression result unused [-Wunused-value]"
//@ diag: 39:3 warning "left operand of comma operator has no effect [-Wunused-value]"
//@ diag: 39:9 warning "expression result unused [-Wunused-value]"
//@ diag: 40:4 warning "expression result unused [-Wunused-value]"
//@ diag: 41:6 warning "left operand of comma operator has no effect [-Wunused-value]"
//@ diag: 41:9 warning "expression result unused [-Wunused-value]"
//@ diag: 42:9 warning "expression result unused [-Wunused-value]"
struct B { int b; };
struct R { int &operator[](int); };
void f(int *a, int i, int *p, B s, B *q, int x, R r, int m[2][3]) {
  a[i];
  i[a];
  *p;
  s.b;
  q->b;
  (void)0;
  a[i] + 1;
  a  [i  ];
  (a[i]);
  -a[i];
  &a[i];
  x;
  1, a[i];
  (*p);
  a[i], x;
  m[0][1];
  r[0];
}
