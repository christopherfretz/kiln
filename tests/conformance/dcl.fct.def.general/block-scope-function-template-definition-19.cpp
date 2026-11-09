// [dcl.fct.def.general]/2: a function template definition in a block.
// Clang 19 (this file) gives "expected expression" at `template` where
// clang 22 gives "function definition is not allowed here" (WP-1091).
//@ expect: compile-fail
//@ strict-diags
//@ requires: clang<22
//@ diag: 9:3 error "expected expression"
void f() {
  template<typename T> void g() {}
}
