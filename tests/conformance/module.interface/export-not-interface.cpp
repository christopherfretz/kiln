// [module.interface]/1: an export-declaration shall appear only in the
// purview of a module interface unit (clang 22 wording; clang 19 says
// "module purview").
//@ expect: compile-fail
//@ std: c++20
//@ requires: clang>=22
//@ strict-diags
//@ diag: 10:1 error "export declaration can only be used within a module interface"
//@ diag: 11:1 error "export declaration can only be used within a module interface"
export int x;
export {
  int y;
}
