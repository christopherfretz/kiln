// [except.handle]/6: a ... handler shall be the last handler for its
// try block.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 7:10 error "catch-all handler must come last"
void f() {
  try {} catch (...) {} catch (int) {}
  try {} catch (int) {} catch (...) {}
}
