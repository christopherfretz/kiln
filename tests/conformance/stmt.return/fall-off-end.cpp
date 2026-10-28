// [stmt.return]/4: flowing off the end of a value-returning function (warned)
//@ expect: compile-ok
//@ strict-diags
//@ diag: 9:1 warning "non-void function does not return a value in all control paths"
//@ diag: 10:10 warning "non-void function does not return a value"
int f(int x) {
  if (x)
    return 1;
}
int g() {}
