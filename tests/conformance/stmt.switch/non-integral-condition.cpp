// [stmt.switch]/2: the condition must be of integral or enumeration type
//@ expect: compile-fail
//@ strict-diags
//@ diag: 6:3 error "statement requires expression of integer type ('double' invalid)"
void f(double d) {
  switch (d) {}
}
