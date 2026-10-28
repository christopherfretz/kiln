// [stmt.switch]/2: case values are distinct
//@ expect: compile-fail
//@ strict-diags
//@ diag: 10:8 error "duplicate case value '1'"
//@ diag: 8:8 note "previous case defined here"
void f(int x) {
  switch (x) {
  case 1:
  case 2:
  case 1:
    break;
  }
}
