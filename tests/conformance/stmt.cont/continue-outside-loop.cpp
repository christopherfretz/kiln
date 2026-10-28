// [stmt.cont]/1: continue needs an enclosing loop
//@ expect: compile-fail
//@ strict-diags
//@ diag: 8:5 error "'continue' statement not in loop statement"
void f(int x) {
  switch (x) {
  case 1:
    continue;
  }
}
