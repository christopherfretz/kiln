// [except.pre]: with exceptions disabled, try and throw are rejected
// (clang -fno-exceptions).
//@ expect: compile-fail
//@ strict-diags
//@ args: -fno-cxx-exceptions
//@ diag: 9:3 error "cannot use 'try' with exceptions disabled"
//@ diag: 10:3 error "cannot use 'throw' with exceptions disabled"
void f() {
  try {} catch (...) {}
  throw 1;
}
