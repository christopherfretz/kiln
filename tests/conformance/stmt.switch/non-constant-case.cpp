// [stmt.switch]/2: case values are constant expressions
//@ expect: compile-fail
//@ strict-diags
//@ diag: 9:8 error "case value is not a constant expression"
//@ diag: 9:8 note "function parameter 'n' with unknown value cannot be used in a constant expression"
//@ diag: 7:19 note "declared here"
void f(int x, int n) {
  switch (x) {
  case n:
    break;
  }
}
