// [expr.new]/8: a class-type array bound with no conversion function,
// own or inherited, is ill-formed (clang 19 wording).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 9:18 error "converting 'DE' to incompatible type 'unsigned long'"
//@ requires: clang<22
struct E {};
struct DE : E {};
int* p = new int[DE()];
