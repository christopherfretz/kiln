// [stmt.label]/1, [stmt.block]/1 (C++23, P2324R2): a label may appear at the
// end of a compound statement and before a declaration.
// WP-185 feature probe.
//@ expect: compile-ok
//@ std: c++23
//@ strict-diags
void f(int x) {
  if (x) goto end;
  goto decl;
decl:
  int y;
  y = x;
  (void)y;
end:
}
