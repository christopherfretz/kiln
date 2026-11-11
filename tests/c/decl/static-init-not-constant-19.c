// C23 6.7.11p4: clang 19 does not evaluate a vector lane access, so a
// static initializer that reads a lane of a vector compound literal is not
// a constant (clang 22 folds it, spec/oven.md "Vectors").
//@ requires: clang<22
//@ expect: compile-fail
//@ strict-diags
//@ diag: 9:9 error "initializer element is not a compile-time constant"
typedef int vec __attribute__((vector_size(8)));
int v = ((vec){ 1, 2 })[1];
