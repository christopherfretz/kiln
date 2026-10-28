// [stmt.pre]/4: the condition is contextually converted to bool
//@ expect: compile-fail
//@ strict-diags
//@ diag: 7:7 error "value of type 'S' is not contextually convertible to 'bool'"
struct S { int v; };
void f(S s) {
  if (s) {}
}
