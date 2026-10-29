// -fno-exceptions: clang's err_exceptions_disabled.
//@ expect: compile-fail
//@ args: -fno-exceptions
//@ strict-diags
//@ diag: 8:3 error "cannot use 'throw' with exceptions disabled"
//@ diag: 9:18 error "cannot use 'throw' with exceptions disabled"
void f(int x) {
  throw;
  (void)(x ? 1 : throw 2);
}
