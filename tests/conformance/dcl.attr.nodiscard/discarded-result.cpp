// [dcl.attr.nodiscard]/4: discarding a nodiscard call is warned
//@ expect: compile-ok
//@ strict-diags
//@ diag: 9:3 warning "ignoring return value of function declared with 'nodiscard' attribute"
//@ diag: 10:3 warning "ignoring return value of function declared with 'nodiscard' attribute: check it"
[[nodiscard]] int must();
[[nodiscard("check it")]] int must2();
void use() {
  must();
  must2();
  (void)must();
}
