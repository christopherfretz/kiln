// [except.pre]/1: a try-block is followed by a handler-seq.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 7:3 error "expected catch"
void f() {
  try {}
  int x;
}
